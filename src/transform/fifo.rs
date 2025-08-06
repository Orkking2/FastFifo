use tracing::{info, instrument, warn};

use crate::transform::{
    Result,
    block::{Block, ReserveState},
    config::{FifoTag, IndexedDrop},
    entry_descriptor::EntryDescriptor,
    error::Error,
    field::Field,
    field::FieldConfig,
    head::{Atomic, AtomicHead, NonAtomicHead},
};
use std::{alloc::Allocator, ptr::NonNull, rc::Rc};

pub(crate) struct FastFifoInner<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator> {
    // num_heads == Tag::num_transformations()
    heads: NonNull<[NonNull<dyn Atomic>]>,
    blocks: NonNull<[Block<Tag, Inner, A>]>,
    num_blocks: usize,
    block_size: usize,
}

unsafe impl<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator> Send
    for FastFifoInner<Tag, Inner, A>
{
}
unsafe impl<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator> Sync
    for FastFifoInner<Tag, Inner, A>
{
}

#[derive(Debug)]
enum AdvanceHeadStatus {
    Busy,
    Success,
}

impl<Tag: FifoTag + 'static, Inner: IndexedDrop<Tag> + Default, A: Allocator>
    FastFifoInner<Tag, Inner, A>
{
    #[instrument(skip(alloc))]
    pub fn new_in(num_blocks: usize, block_size: usize, alloc: A) -> Self {
        let rc_alloc = Rc::new(alloc);

        Self {
            heads: unsafe {
                NonNull::new_unchecked(Box::into_raw({
                    let mut vec = Vec::new_in(rc_alloc.as_ref());
                    vec.reserve(Tag::num_transformations());

                    vec.extend((0..Tag::num_transformations()).map(|i| {
                        let tag = Tag::try_from(i).unwrap();

                        let field = Field::from_parts(num_blocks, 0, 0);

                        info!("Head[{i}] = {field:?}");

                        NonNull::new_unchecked(Box::into_raw(if tag.is_atomic() {
                            Box::new_in(AtomicHead::from(field), rc_alloc.as_ref())
                                as Box<dyn Atomic, _>
                        } else {
                            Box::new_in(NonAtomicHead::from(field), rc_alloc.as_ref())
                        }))
                    }));

                    vec.into_boxed_slice()
                }))
            },
            blocks: unsafe {
                NonNull::new_unchecked(Box::into_raw({
                    let mut vec = Vec::new_in(rc_alloc.as_ref());
                    vec.reserve(num_blocks);

                    vec.extend((0..num_blocks).map(|i| {
                        info!("Init block {i}");
                        Block::new_in(block_size, rc_alloc.clone())
                    }));

                    vec.into_boxed_slice()
                }))
            },
            num_blocks,
            block_size,
        }
    }
}

impl<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator> FastFifoInner<Tag, Inner, A> {
    fn get_head(&self, tag: Tag) -> &dyn Atomic {
        // Safety: this pointer can be turned into a reference because I said so.
        unsafe { self.heads.as_ref().get(tag.into()).unwrap().as_ref() }
    }

    #[instrument(skip(self, tag))]
    fn get_block(&self, tag: Tag) -> (Field, &Block<Tag, Inner, A>) {
        let head = self.get_head(tag).load();
        info!(?head);

        unsafe { (head.clone(), &self.blocks.as_ref()[head.get_index()]) }
    }

    #[instrument(skip(self, tag))]
    fn advance_head(&self, head: Field, tag: Tag) -> AdvanceHeadStatus {
        let (next_current, next_chasing) = unsafe {
            self.blocks.as_ref()[(head.get_index() + 1) % self.num_blocks].get_current_chasing(tag)
        };

        let chasing_give = next_chasing.load_give();

        info!(?chasing_give);

        if let AdvanceHeadStatus::Success = if chasing_give.get_index() >= self.num_blocks {
            info!("Success (early)");

            // Guaranteed to be able to advance to next block, early escape
            AdvanceHeadStatus::Success
        } else {
            // `give`s are AcqRel symantics, the release of the previous `give` guarantees
            // that `take.index` (which is incremented previously to the `give`s release)
            // is at least `give.index`, that is, chasing_give.index <= chasing_take.index is always true.
            let chasing_take = next_chasing.load_take();

            info!(?chasing_take);

            if chasing_take.get_index() > chasing_give.get_index() {
                warn!("Busy");

                // The pair we are chasing is currently writing
                // We do not know in which slot they are writing
                // We must assume that every entry is garbage
                AdvanceHeadStatus::Busy
            } else {
                info!("Success");

                // MUST be chasing_take == chasing_give, the valid state to advance this head
                AdvanceHeadStatus::Success
            }
        } {
            // Success, update atomics in nblk and cached head

            let new_next_current = Field::from(FieldConfig {
                index_max: self.block_size,
                version: head.get_version() + 1,
                index: 0,
            });
            info!(?new_next_current);

            let (old_give, old_take) = next_current.fetch_max_both(new_next_current);
            info!(?old_give, ?old_take);

            let head_vsn_inc_add = head.version_inc_add(1);
            info!(?head_vsn_inc_add);

            let old_head = self.get_head(tag).max(head_vsn_inc_add);
            info!(?old_head);

            // Forward success
            AdvanceHeadStatus::Success
        } else {
            // Forward busy
            AdvanceHeadStatus::Busy
        }
    }

    #[instrument(skip(self))]
    pub fn get_entry(&self, tag: Tag) -> Result<EntryDescriptor<'_, Tag, Inner, A>> {
        //         v [2].give (1)
        //         |         v [2].take (2)
        //         |         |           v [1].give (3)
        //         |         |           |            v [1].take (4)
        //         |         |           |            |          v [0].give (5)
        //         |         |           |            |          |          v [0].take (6)
        // [Uninit, Reserved, Post_Trans, Trans_Alloc, Pre_Trans, Allocated, Uninit] ->

        loop {
            let (head, block) = self.get_block(tag);

            match block.reserve_in_layer(tag) {
                ReserveState::Success(entry_descriptor) => {
                    info!(
                        "Success, block[{}][{}]",
                        head.get_index(),
                        entry_descriptor.index,
                    );
                    break Ok(entry_descriptor);
                }
                ReserveState::NotAvailable => {
                    break Err(Error::NotAvailable);
                }
                ReserveState::Busy => {
                    break Err(Error::Busy);
                }
                ReserveState::BlockDone => match self.advance_head(head, tag) {
                    AdvanceHeadStatus::Busy => {
                        break Err(Error::Busy);
                    }
                    AdvanceHeadStatus::Success => {
                        continue;
                    }
                },
            }
        }
    }
}
