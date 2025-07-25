use crate::{
    field::FieldConfig,
    transform::{
        Result,
        block::{Block, ReserveState},
        config::{FifoTag, IndexedDrop},
        entry_descriptor::EntryDescriptor,
        error::Error,
        head::{Atomic, AtomicHead, NonAtomicHead},
        wide_field::WideField,
    },
};
use std::array;

// type Tag: FifoTag;
// type Inner: IndexedDrop + Default;
// const NUM_TRANSFORMATIONS: usize;
// const NUM_BLOCKS: usize;
// const BLOCK_SIZE: usize;

pub(crate) struct FastFifoInner<
    Tag: FifoTag,
    Inner: IndexedDrop<Tag> + Default,
    const NUM_BLOCKS: usize,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> where
    [(); NUM_BLOCKS]:,
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
    heads: [Box<dyn Atomic<NUM_BLOCKS, Tag>>; NUM_TRANSFORMATIONS],
    blocks: [Block<Tag, Inner, BLOCK_SIZE, NUM_TRANSFORMATIONS>; NUM_BLOCKS],
}

unsafe impl<
    Tag: FifoTag,
    Inner: IndexedDrop<Tag> + Default,
    const NUM_BLOCKS: usize,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> Send for FastFifoInner<Tag, Inner, NUM_BLOCKS, BLOCK_SIZE, NUM_TRANSFORMATIONS>
where
    [(); NUM_BLOCKS]:,
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
}
unsafe impl<
    Tag: FifoTag,
    Inner: IndexedDrop<Tag> + Default,
    const NUM_BLOCKS: usize,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> Sync for FastFifoInner<Tag, Inner, NUM_BLOCKS, BLOCK_SIZE, NUM_TRANSFORMATIONS>
where
    [(); NUM_BLOCKS]:,
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
}

impl<
    Tag: FifoTag + 'static,
    Inner: IndexedDrop<Tag> + Default,
    const NUM_BLOCKS: usize,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> Default for FastFifoInner<Tag, Inner, NUM_BLOCKS, BLOCK_SIZE, NUM_TRANSFORMATIONS>
where
    [(); NUM_BLOCKS]:,
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
    fn default() -> Self {
        Self::new()
    }
}

enum AdvanceHeadStatus {
    Busy,
    Success,
}

impl<
    Tag: FifoTag + 'static,
    Inner: IndexedDrop<Tag> + Default,
    const NUM_BLOCKS: usize,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> FastFifoInner<Tag, Inner, NUM_BLOCKS, BLOCK_SIZE, NUM_TRANSFORMATIONS>
where
    [(); NUM_BLOCKS]:,
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
    pub fn new() -> Self {
        Self {
            heads: array::from_fn(|i| {
                let tag = Tag::try_from(i).unwrap();

                if tag.is_consumer(NUM_TRANSFORMATIONS - 1) {
                    if tag.is_atomic() {
                        Box::new(AtomicHead::full(tag)) as Box<dyn Atomic<NUM_BLOCKS, Tag>>
                    } else {
                        Box::new(NonAtomicHead::full(tag))
                    }
                } else {
                    if tag.is_atomic() {
                        Box::new(AtomicHead::new(tag)) as Box<dyn Atomic<NUM_BLOCKS, Tag>>
                    } else {
                        Box::new(NonAtomicHead::new(tag))
                    }
                }
            }),
            blocks: array::from_fn(|_| Block::full_consumer()),
        }
    }

    fn get_head(&self, tag: Tag) -> &dyn Atomic<NUM_BLOCKS, Tag> {
        &*self.heads[tag.into()]
    }

    fn get_block(
        &self,
        tag: Tag,
    ) -> (
        WideField<NUM_BLOCKS, Tag>,
        &Block<Tag, Inner, BLOCK_SIZE, NUM_TRANSFORMATIONS>,
    ) {
        let head = self.get_head(tag).load();
        (head.clone(), &self.blocks[head.get_index()])
    }

    fn advance_head(&self, head: WideField<NUM_BLOCKS, Tag>) -> AdvanceHeadStatus {
        let (next_current, next_chasing) =
            self.blocks[(head.get_index() + 1) % NUM_BLOCKS].get_current_chasing(head.get_tag());

        let chasing_give = next_chasing.load_give();

        if let AdvanceHeadStatus::Success = if chasing_give.get_index() >= NUM_BLOCKS {
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
                version: head.get_version() + 1,
                ..Default::default()
            });

            self.get_head(head.get_tag()).max(head.version_inc_add(1));

            // Forward success
            AdvanceHeadStatus::Success
        } else {
            // Forward busy
            AdvanceHeadStatus::Busy
        }
    }

    pub fn get_entry(
        &self,
        tag: Tag,
    ) -> Result<EntryDescriptor<'_, Tag, Inner, BLOCK_SIZE, NUM_TRANSFORMATIONS>> {
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

impl<
    Tag: FifoTag,
    Inner: IndexedDrop<Tag> + Default,
    const NUM_BLOCKS: usize,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> Drop for FastFifoInner<Tag, Inner, NUM_BLOCKS, BLOCK_SIZE, NUM_TRANSFORMATIONS>
where
    [(); NUM_BLOCKS]:,
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
    fn drop(&mut self) {
        for (i, block) in self.blocks.iter_mut().enumerate() {}
    }
}
