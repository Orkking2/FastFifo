use self::{
    entries::{ConsumingEntry, ProducingEntry},
    error::Error,
};
use fifo::FastFifoInner;
use std::{fmt::Debug, sync::Arc};

mod atomic;
mod block;
mod entries;
mod error;
mod fifo;
#[cfg(test)]
mod test;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Clone)]
pub struct FastFifo<T>(
    Arc<FastFifoInner<T>>,
);

/// This type allows for the construction of a FastFifo from a CAPACITY instead of a NUM_BLOCKS.
// pub struct CohortFastFifo<T, const CAPACITY: usize, const BLOCK_SIZE: usize>(PhantomData<T>);

// pub const fn ceiling_div(lhs: usize, rhs: usize) -> usize {
//     lhs / rhs + if lhs % rhs != 0 { 1 } else { 0 }
// }

// impl<T, const CAPACITY: usize, const BLOCK_SIZE: usize> CohortFastFifo<T, CAPACITY> {
//     /// At least enough blocks to get CAPACITY size
//     pub fn new() -> FastFifo<T, { ceiling_div(CAPACITY) }> {
//         FastFifo::new()
//     }
// }

impl<T> FastFifo<T> {
    pub fn new(num_blocks: usize, block_size: usize) -> Self {
        Self(Arc::new(FastFifoInner::new(num_blocks, block_size)))
    }

    pub fn try_get_producer_entry(&self) -> Result<ProducingEntry<'_, T>> {
        self.0.get_producer_entry()
    }

    pub fn push_in_place<F: FnOnce(*mut T)>(&self, producer: F) -> Result<()> {
        self.0.push_in_place(producer)
    }

    pub fn push(&self, val: T) -> Result<()> {
        self.0.push(val)
    }

    pub fn try_get_consumer_entry(&self) -> Result<ConsumingEntry<'_, T>> {
        self.0.get_consumer_entry()
    }

    pub fn pop_in_place<F: FnOnce(*mut T)>(&self, consumer: F) -> Result<()> {
        self.0.pop_in_place(consumer)
    }

    pub fn pop(&self) -> Result<T> {
        self.0.pop()
    }
}

impl<T: Debug> Debug
    for FastFifo<T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0.as_ref())
    }
}
