use crate::{
    field::FieldConfig,
    transform::{
        IsInOutUnion, Result,
        block::{Block, ReserveState},
        entries::{ConsumingEntry, EntryDescriptor, ProducingEntry, TransformingEntry},
        error::Error,
        head::{Atomic, AtomicHead, NonAtomicHead},
        layer::Layer,
        wide_field::WideField,
    },
};
use std::array;

pub(crate) struct FastFifoInner<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> {
    heads: [Box<dyn Atomic<NUM_BLOCKS>>; 3],
    blocks: [Block<T, BLOCK_SIZE>; NUM_BLOCKS],
}

#[rustfmt::skip]
unsafe impl<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Send for FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE> {}
#[rustfmt::skip]
unsafe impl<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Sync for FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE> {}

impl<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Default
    for FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

enum AdvanceHeadStatus {
    Busy,
    Success,
}

impl<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>
    FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE>
{
    pub fn new() -> Self {
        Self {
            heads: array::from_fn(|i| {
                let layer = Layer::try_from(i).unwrap();

                if layer == Layer::Transformer {
                    Box::new(AtomicHead::new(layer)) as Box<dyn Atomic<NUM_BLOCKS>>
                } else {
                    Box::new(NonAtomicHead::new(layer))
                }
            }),
            blocks: array::from_fn(|_| Default::default()),
        }
    }

    fn block_from_head(
        &self,
        head: &dyn Atomic<NUM_BLOCKS>,
    ) -> (WideField<NUM_BLOCKS>, &Block<T, BLOCK_SIZE>) {
        let head = head.load();
        (head, &self.blocks[head.get_index()])
    }

    fn get_head(&self, layer: Layer) -> &dyn Atomic<NUM_BLOCKS> {
        &*self.heads[layer as usize]
    }

    fn advance_head(&self, head: WideField<NUM_BLOCKS>) -> AdvanceHeadStatus {
        let (next_current, next_chasing) =
            self.blocks[(head.get_index() + 1) % NUM_BLOCKS].get_current_chasing(head.get_layer());

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
                // We must assume that the 0th entry is garbage and retry
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

            self.get_head(head.get_layer()).max(head.version_inc_add(1));

            // Forward success
            AdvanceHeadStatus::Success
        } else {
            // Forward busy
            AdvanceHeadStatus::Busy
        }
    }

    fn get_entry(&self, layer: Layer) -> Result<EntryDescriptor<'_, T, BLOCK_SIZE>> {
        loop {
            let (head, block) = self.block_from_head(self.get_head(layer));

            match block.reserve_in_layer(layer) {
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

    pub fn get_producer_entry(&self) -> Result<ProducingEntry<'_, T, BLOCK_SIZE>> {
        self.get_entry(Layer::Producer)
            .map(|entry_descriptor| ProducingEntry(entry_descriptor))
    }

    pub fn get_consumer_entry(&self) -> Result<ConsumingEntry<'_, T, BLOCK_SIZE>> {
        self.get_entry(Layer::Consumer)
            .map(|entry_descriptor| ConsumingEntry(entry_descriptor))
    }

    pub fn get_transformer_entry(&self) -> Result<TransformingEntry<'_, T, BLOCK_SIZE>> {
        self.get_entry(Layer::Transformer)
            .map(|entry_descriptor| TransformingEntry(entry_descriptor))
    }
}

impl<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Drop
    for FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE>
{
    fn drop(&mut self) {
        if <T as IsInOutUnion>::VALUE {
            // Drop each element of the union specifically
        } else {
            // Drop every valid T as if it's a T
        }
    }
}
