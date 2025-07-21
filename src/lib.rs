#![feature(unsafe_cell_access)]
// ! temp
#![feature(thread_sleep_until)]

use crate::{
    entries::{ConsumingEntry, ProducingEntry},
    error::Error,
    fifo::FastFifoInner,
};
use std::{fmt::Debug, sync::Arc};

mod atomic;
mod block;
mod error;
mod field;
mod fifo;

#[cfg(test)]
mod test;

pub mod entries;

pub type Result<T> = std::result::Result<T, Error>;

#[repr(transparent)]
#[derive(Clone)]
pub struct FastFifo<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>(
    Arc<FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE>>,
);

impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> FastFifo<T, NUM_BLOCKS, BLOCK_SIZE> {
    pub fn new() -> Self {
        Self(Arc::new(FastFifoInner::new()))
    }

    pub const fn capacity() -> usize {
        FastFifoInner::<T, NUM_BLOCKS, BLOCK_SIZE>::capacity()
    }

    pub fn try_get_producer_entry(&self) -> Result<ProducingEntry<'_, T, BLOCK_SIZE>> {
        self.0.try_get_producer_entry()
    }

    pub fn push_in_place<F: FnOnce(*mut T)>(&self, producer: F) -> Result<()> {
        self.0.push_in_place(producer)
    }

    pub fn push(&self, val: T) -> Result<()> {
        self.0.push(val)
    }

    pub fn try_get_consumer_entry(&self) -> Result<ConsumingEntry<'_, T, BLOCK_SIZE>> {
        self.0.try_get_consumer_entry()
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
