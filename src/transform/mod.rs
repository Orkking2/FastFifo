//         v Consumed (1)
//         |         v Reserved (2)
//         |         |           v Trans_committed (3)
//         |         |           |            v Trans_allocated (4)
//         |         |           |            |          v Committed (5)
//         |         |           |            |          |          v Allocated (6)
// [Uninit, Reserved, Post_Trans, Trans_Alloc, Pre_Trans, Allocated, Uninit] ->

use crate::transform::{
    config::{FifoTag, IndexedDrop, TaggedClone},
    entry_descriptor::EntryDescriptor,
    error::Error,
    fifo::FastFifoInner,
};
use std::sync::Arc;

pub use fastfifoprocmacro::generate_union;

pub mod config;
pub mod entry_descriptor;
pub mod error;

pub type Result<T> = std::result::Result<T, Error>;

mod atom_pair;
mod block;
mod fifo;
mod head;
mod wide_field;

pub struct FastFifo<
    Tag: FifoTag,
    Inner: IndexedDrop<Tag> + Default,
    const NUM_BLOCKS: usize,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
>(Arc<FastFifoInner<Tag, Inner, NUM_BLOCKS, BLOCK_SIZE, NUM_TRANSFORMATIONS>>)
where
    [(); NUM_BLOCKS]:,
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:;

impl<
    Tag: FifoTag,
    Inner: IndexedDrop<Tag> + Default,
    const NUM_BLOCKS: usize,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> TaggedClone<Tag> for FastFifo<Tag, Inner, NUM_BLOCKS, BLOCK_SIZE, NUM_TRANSFORMATIONS>
where
    [(); NUM_BLOCKS]:,
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
    fn unchecked_clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<
    Tag: FifoTag + 'static,
    Inner: IndexedDrop<Tag> + Default,
    const NUM_BLOCKS: usize,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> FastFifo<Tag, Inner, NUM_BLOCKS, BLOCK_SIZE, NUM_TRANSFORMATIONS>
where
    [(); NUM_BLOCKS]:,
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
    pub fn new() -> Self {
        Self(Arc::new(FastFifoInner::new()))
    }
}

impl<
    Tag: FifoTag + 'static,
    Inner: IndexedDrop<Tag> + Default,
    const NUM_BLOCKS: usize,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> FastFifo<Tag, Inner, NUM_BLOCKS, BLOCK_SIZE, NUM_TRANSFORMATIONS>
where
    [(); NUM_BLOCKS]:,
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
    pub fn get_entry(
        &self,
        tag: Tag,
    ) -> Result<EntryDescriptor<'_, Tag, Inner, BLOCK_SIZE, NUM_TRANSFORMATIONS>> {
        self.0.get_entry(tag)
    }
}
