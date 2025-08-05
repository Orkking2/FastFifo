use std::alloc::Allocator;
use crate::transform::{
    block::Block,
    config::{FifoTag, IndexedDrop},
};

pub struct EntryDescriptor<'a, Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator> {
    pub(crate) block: &'a Block<Tag, Inner, A>,
    pub(crate) index: usize,
    pub(crate) tag: Tag,
}

impl<'a, Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator> EntryDescriptor<'a, Tag, Inner, A> {
    pub fn modify_t_in_place<F: FnOnce(*mut Inner)>(&mut self, modifier: F) {
        modifier(unsafe { self.block.get_ptr(self.index) })
    }
}

impl<'a, Tag: FifoTag, Inner: IndexedDrop<Tag> + Default, A: Allocator> Drop for EntryDescriptor<'a, Tag, Inner, A> {
    fn drop(&mut self) {
        self.block.get_atomics(self.tag).incr_give();
    }
}
