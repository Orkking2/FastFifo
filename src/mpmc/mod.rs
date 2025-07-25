use crate::{
    Result,
    entries::{ConsumingEntry, ProducingEntry},
};
use fifo::FastFifoInner;
use std::{fmt::Debug, marker::PhantomData, sync::Arc};

mod fifo;
#[cfg(test)]
mod test;

#[derive(Clone)]
pub struct FastFifo<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>(
    Arc<FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE>>,
);

/// This type allows for the construction of a FastFifo from a CAPACITY instead of a NUM_BLOCKS.
// pub struct CohortFastFifo<T, const CAPACITY: usize, const BLOCK_SIZE: usize>(PhantomData<T>);

// pub const fn ceiling_div(lhs: usize, rhs: usize) -> usize {
//     lhs / rhs + if lhs % rhs != 0 { 1 } else { 0 }
// }

// impl<T, const CAPACITY: usize, const BLOCK_SIZE: usize> CohortFastFifo<T, CAPACITY, BLOCK_SIZE> {
//     /// At least enough blocks to get CAPACITY size
//     pub fn new() -> FastFifo<T, { ceiling_div(CAPACITY, BLOCK_SIZE) }, BLOCK_SIZE> {
//         FastFifo::new()
//     }
// }

impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> FastFifo<T, NUM_BLOCKS, BLOCK_SIZE> {
    pub fn new() -> Self {
        Self(Arc::new(FastFifoInner::new()))
    }

    pub const fn capacity() -> usize {
        FastFifoInner::<T, NUM_BLOCKS, BLOCK_SIZE>::capacity()
    }

    pub fn try_get_producer_entry(&self) -> Result<ProducingEntry<'_, T, BLOCK_SIZE>> {
        self.0.get_producer_entry()
    }

    pub fn push_in_place<F: FnOnce(*mut T)>(&self, producer: F) -> Result<()> {
        self.0.push_in_place(producer)
    }

    pub fn push(&self, val: T) -> Result<()> {
        self.0.push(val)
    }

    pub fn try_get_consumer_entry(&self) -> Result<ConsumingEntry<'_, T, BLOCK_SIZE>> {
        self.0.get_consumer_entry()
    }

    pub fn pop_in_place<F: FnOnce(*mut T)>(&self, consumer: F) -> Result<()> {
        self.0.pop_in_place(consumer)
    }

    pub fn pop(&self) -> Result<T> {
        self.0.pop()
    }
}

impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Default
    for FastFifo<T, NUM_BLOCKS, BLOCK_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Debug, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Debug
    for FastFifo<T, NUM_BLOCKS, BLOCK_SIZE>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0.as_ref())
    }
}
