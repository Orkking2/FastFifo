use crate::{
    Result,
    config::{FifoTag, IndexedDrop, TaggedClone},
    entry_descriptor::EntryDescriptor,
    fifo_inner::FastFifoInner,
};
use std::{
    // alloc::{Allocator, Global},
    sync::Arc,
};

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