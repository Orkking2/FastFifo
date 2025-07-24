use crate::{
    field::FieldConfig,
    transform::{
        Result,
        block::{Block, ReserveState},
        config::FifoConfig,
        config::FifoTag,
        entry_descriptor::EntryDescriptor,
        error::Error,
        head::{Atomic, AtomicHead, NonAtomicHead},
        wide_field::WideField,
    },
};
use std::array;

pub(crate) struct FastFifoInner<Config: FifoConfig>
where
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    heads:
        [Box<dyn Atomic<{ <Config as FifoConfig>::NUM_BLOCKS }, <Config as FifoConfig>::Tag>>; 3],
    blocks: [Block<Config>; <Config as FifoConfig>::NUM_BLOCKS],
}

unsafe impl<Config: FifoConfig> Send for FastFifoInner<Config>
where
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
}
unsafe impl<Config: FifoConfig> Sync for FastFifoInner<Config>
where
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
}

impl<Config: FifoConfig + 'static> Default for FastFifoInner<Config>
where
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    fn default() -> Self {
        Self::new()
    }
}

enum AdvanceHeadStatus {
    Busy,
    Success,
}

impl<Config: FifoConfig + 'static> FastFifoInner<Config>
where
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    pub fn new() -> Self {
        Self {
            heads: array::from_fn(|i| {
                let tag = <Config as FifoConfig>::Tag::try_from(i).unwrap();

                if tag.is_consumer(<Config as FifoConfig>::NUM_TRANSFORMATIONS - 1) {
                    if tag.is_atomic() {
                        Box::new(AtomicHead::full(tag))
                            as Box<
                                dyn Atomic<
                                        { <Config as FifoConfig>::NUM_BLOCKS },
                                        <Config as FifoConfig>::Tag,
                                    >,
                            >
                    } else {
                        Box::new(NonAtomicHead::full(tag))
                    }
                } else {
                    if tag.is_atomic() {
                        Box::new(AtomicHead::new(tag))
                            as Box<
                                dyn Atomic<
                                        { <Config as FifoConfig>::NUM_BLOCKS },
                                        <Config as FifoConfig>::Tag,
                                    >,
                            >
                    } else {
                        Box::new(NonAtomicHead::new(tag))
                    }
                }
            }),
            blocks: array::from_fn(|_| Block::full_consumer()),
        }
    }

    fn get_head(
        &self,
        tag: <Config as FifoConfig>::Tag,
    ) -> &dyn Atomic<{ <Config as FifoConfig>::NUM_BLOCKS }, <Config as FifoConfig>::Tag> {
        &*self.heads[tag.into()]
    }

    fn get_block(
        &self,
        tag: <Config as FifoConfig>::Tag,
    ) -> (
        WideField<{ <Config as FifoConfig>::NUM_BLOCKS }, <Config as FifoConfig>::Tag>,
        &Block<Config>,
    ) {
        let head = self.get_head(tag).load();
        (head.clone(), &self.blocks[head.get_index()])
    }

    fn advance_head(
        &self,
        head: WideField<{ <Config as FifoConfig>::NUM_BLOCKS }, <Config as FifoConfig>::Tag>,
    ) -> AdvanceHeadStatus {
        let (next_current, next_chasing) = self.blocks
            [(head.get_index() + 1) % { <Config as FifoConfig>::NUM_BLOCKS }]
        .get_current_chasing(head.get_tag());

        let chasing_give = next_chasing.load_give();

        if let AdvanceHeadStatus::Success =
            if chasing_give.get_index() >= { <Config as FifoConfig>::NUM_BLOCKS } {
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
            }
        {
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
        tag: <Config as FifoConfig>::Tag,
    ) -> Result<EntryDescriptor<'_, Config>> {
        //         v [2].give (1)
        //         |         v [2].take (2)
        //         |         |           v [1].give (3)
        //         |         |           |            v [1].take (4)
        //         |         |           |            |          v [0].give (5)
        //         |         |           |            |          |          v [0].take (6)
        // [Uninit, Reserved, Post_Trans, Trans_Alloc, Pre_Trans, Allocated, Uninit] ->

        loop {
            let (head, block) = self.get_block(tag);

            match block.reserve_in_tag(tag) {
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

// impl<Config: FifoConfig> Drop for FastFifoInner<Config>
// where
//     [(); <Config as FifoConfig>::NUM_BLOCKS]:,
//     [(); <Config as FifoConfig>::BLOCK_SIZE]:,
//     [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
// {
//     fn drop(&mut self) {
//         for (i, block) in self.blocks.iter_mut().enumerate() {
//

//         }
//     }
// }
