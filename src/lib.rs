// #![feature(allocator_api)]

extern crate self as fastfifo;

use crate::{
    config::{FifoTag, IndexedDrop, TaggedClone},
    entry_descriptor::EntryDescriptor,
    error::Error,
    fifo::FastFifoInner,
};
use std::{
    // alloc::{Allocator, Global},
    sync::Arc,
};

pub use fastfifoprocmacro::generate_union;

pub mod config;
pub mod entry_descriptor;
pub mod error;
pub mod mpmc;

pub type Result<T> = std::result::Result<T, Error>;

mod atom_pair;
mod block;
mod field;
mod fifo;
mod head;

pub struct FastFifo<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default /*A: Allocator = Global*/>(
    Arc<FastFifoInner<Tag, Inner /*A*/>>,
);

impl<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default /*, A: Allocator*/> TaggedClone<Tag>
    for FastFifo<Tag, Inner /*A*/>
{
    fn unchecked_clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Tag: FifoTag + 'static, Inner: IndexedDrop<Tag> + Default> FastFifo<Tag, Inner> {
    pub fn new(num_blocks: usize, block_size: usize) -> Self {
        Self(Arc::new(FastFifoInner::new_in(num_blocks, block_size)))
        // Self::new_in(num_blocks, block_size, Global)
    }
}

// impl<Tag: FifoTag + 'static, Inner: IndexedDrop<Tag> + Default, /*A: Allocator*/>
//     FastFifo<Tag, Inner, A>
// {
//     pub fn new_in(num_blocks: usize, block_size: usize, alloc: A) -> Self {
//         Self(Arc::new(FastFifoInner::new_in(
//             num_blocks, block_size, alloc,
//         )))
//     }
// }

impl<Tag: FifoTag, Inner: IndexedDrop<Tag> + Default /*A: Allocator*/>
    FastFifo<Tag, Inner /*A*/>
{
    pub fn get_entry(&self, tag: Tag) -> Result<EntryDescriptor<'_, Tag, Inner /*A*/>> {
        self.0.get_entry(tag)
    }
}
