//         v Consumed (1)
//         |         v Reserved (2)
//         |         |           v Trans_committed (3)
//         |         |           |            v Trans_allocated (4)
//         |         |           |            |          v Committed (5)
//         |         |           |            |          |          v Allocated (6)
// [Uninit, Reserved, Post_Trans, Trans_Alloc, Pre_Trans, Allocated, Uninit] ->

use crate::{
    transform::{
        config::{FifoConfig, FifoTag, TaggedClone},
        entry_descriptor::EntryDescriptor,
        error::Error,
        fifo::FastFifoInner,
    },
};
use std::sync::Arc;

#[doc(hidden)]
pub mod __exported_macros {
    pub use paste::paste;
}

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

pub struct FastFifo<Config: FifoConfig>(Arc<FastFifoInner<Config>>)
where
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:;

impl<Config: FifoConfig> TaggedClone for FastFifo<Config>
where
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    type Tag = <Config as FifoConfig>::Tag;

    fn unchecked_clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Config: FifoConfig + 'static> FastFifo<Config>
where
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    pub fn new() -> Self {
        Self(Arc::new(FastFifoInner::new()))
    }
}

impl<Config: FifoConfig + 'static> FastFifo<Config>
where
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    pub fn get_entry(
        &self,
        tag: <Config as FifoConfig>::Tag,
    ) -> Result<EntryDescriptor<'_, Config>> {
        self.0.get_entry(tag)
    }
}
