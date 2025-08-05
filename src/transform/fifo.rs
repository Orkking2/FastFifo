use crate::transform::{
    Result,
    block::{Block, ReserveState},
    config::{FifoTag, IndexedDrop},
    entry_descriptor::EntryDescriptor,
    error::Error,
    field::Field,
    field::FieldConfig,
    head::{Atomic, AtomicHead, NonAtomicHead},
    wide_field::WideField,
};
use std::{alloc::Allocator, ptr::NonNull, rc::Rc};

pub(crate) struct FastFifoInner<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator> {
    heads: NonNull<[NonNull<dyn Atomic<Tag>>]>,
    num_blocks: usize,
    blocks: NonNull<[Block<Tag, Inner, A>]>,
}

unsafe impl<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator> Send
    for FastFifoInner<Tag, Inner, A>
{
}
unsafe impl<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator> Sync
    for FastFifoInner<Tag, Inner, A>
{
}

enum AdvanceHeadStatus {
    Busy,
    Success,
}

impl<Tag: FifoTag + 'static, Inner: IndexedDrop<Tag> + Default, A: Allocator>
    FastFifoInner<Tag, Inner, A>
{
    pub fn new_in(num_blocks: usize, block_size: usize, alloc: A) -> Self {
        let rc_alloc = Rc::new(alloc);

        Self {
            heads: unsafe {
                NonNull::new_unchecked(Box::into_raw({
                    let mut vec = Vec::new_in(rc_alloc.as_ref());
                    vec.reserve(Tag::num_transformations());

                    vec.extend((0..Tag::num_transformations()).map(|i| {
                        let tag = Tag::try_from(i).unwrap();

                        let field = WideField::from_parts(
                            Field::from_parts(
                                num_blocks,
                                0,
                                if tag.into() == Tag::producer().chases().into() {
                                    num_blocks - 1
                                } else {
                                    0
                                },
                            ),
                            tag,
                        );

                        // Safety: Same as above.
                        NonNull::new_unchecked(Box::into_raw(if tag.is_atomic() {
                            Box::new_in(AtomicHead::from(field), rc_alloc.as_ref())
                                as Box<dyn Atomic<Tag>, _>
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

                    vec.extend(
                        (0..num_blocks).map(|_| Block::new(block_size, true, rc_alloc.clone())),
                    );

                    vec.into_boxed_slice()
                }))
            },
            num_blocks,
        }
    }
}

impl<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator>
    FastFifoInner<Tag, Inner, A>
{
    fn get_head(&self, tag: Tag) -> &dyn Atomic<Tag> {
        // Safety: this pointer can be turned into a reference because I said so.
        unsafe { self.heads.as_ref().get(tag.into()).unwrap().as_ref() }
    }

    fn get_block(&self, tag: Tag) -> (WideField<Tag>, &Block<Tag, Inner, A>) {
        let head = self.get_head(tag).load();

        unsafe { (head.clone(), &self.blocks.as_ref()[head.get_index()]) }
    }

    fn advance_head(&self, head: WideField<Tag>) -> AdvanceHeadStatus {
        let (next_current, next_chasing) = unsafe {
            self.blocks.as_ref()[(head.get_index() + 1) % self.num_blocks]
                .get_current_chasing(head.get_tag())
        };

        let chasing_give = next_chasing.load_give();

        if let AdvanceHeadStatus::Success = if chasing_give.get_index() >= self.num_blocks {
            // Guaranteed to be able to advance to next block, early escape
            AdvanceHeadStatus::Success
        } else {
            // `give`s are AcqRel symantics, the release of the previous `give` guarantees
            // that `take.index` (which is incremented previously to the `give`s release)
            // is at least `give.index`, that is, chasing_give.index <= chasing_take.index is always true.
            let chasing_take = next_chasing.load_take();

            if chasing_take.get_index() > chasing_give.get_index() {
                // The pair we are chasing is currently writing
                // We do not know in which slot they are writing
                // We must assume that every entry is garbage
                AdvanceHeadStatus::Busy
            } else {
                // MUST be chasing_take == chasing_give, the valid state to advance this head
                AdvanceHeadStatus::Success
            }
        } {
            // Success, update atomics in nblk and cached head

            next_current.fetch_max_both(FieldConfig {
                index_max: self.num_blocks,
                version: head.get_version() + 1,
                index: 0,
            });

            self.get_head(head.get_tag()).max(head.version_inc_add(1));

            // Forward success
            AdvanceHeadStatus::Success
        } else {
            // Forward busy
            AdvanceHeadStatus::Busy
        }
    }

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

            // std::thread::sleep(std::time::Duration::from_millis(10));

            match block.reserve_in_layer(tag) {
                ReserveState::Success(entry_descriptor) => break Ok(entry_descriptor),
                ReserveState::NotAvailable => break Err(Error::NotAvailable),
                ReserveState::Busy => break Err(Error::Busy),
                ReserveState::BlockDone => match self.advance_head(head) {
                    AdvanceHeadStatus::Busy => break Err(Error::Busy),
                    AdvanceHeadStatus::Success => continue,
                },
            }
        }
    }
}

// impl<
//     Tag: FifoTag,
//     Inner: IndexedDrop<Tag> + Default,
//     const NUM_BLOCKS: usize,
//     const BLOCK_SIZE: usize,
//     const NUM_TRANSFORMATIONS: usize,
// > Drop for FastFifoInner<Tag, Inner>
// where
//     [(); NUM_BLOCKS]:,
//     [(); BLOCK_SIZE]:,
//     [(); NUM_TRANSFORMATIONS]:,
// {
//     fn drop(&mut self) {
//         for (_i, block) in self.blocks.iter_mut().enumerate() {
//             block.drop()
//         }
//     }
// }
