use crate::{
    block::Block,
    config::{FifoTag, IndexedDrop},
};
// use std::alloc::{Allocator, Global};

pub struct EntryDescriptor<'a, Tag: FifoTag, Inner: IndexedDrop<Tag> /*A: Allocator = Global*/> {
    pub(crate) block: &'a Block<Tag, Inner /*A*/>,
    pub(crate) index: usize,
    pub(crate) tag: Tag,
}

impl<'a, Tag: FifoTag, Inner: IndexedDrop<Tag> /*, A: Allocator*/>
    EntryDescriptor<'a, Tag, Inner /*A*/>
{
    pub fn modify_t_in_place<F: FnOnce(*mut Inner)>(&mut self, modifier: F) {
        #[cfg(not(loom))]
        modifier(self.block.get_ptr(self.index));
        #[cfg(loom)]
        self.block.get_ptr(self.index).with(modifier);
    }
}

impl<'a, Tag: FifoTag, Inner: IndexedDrop<Tag> /*, A: Allocator*/> Drop
    for EntryDescriptor<'a, Tag, Inner /*A*/>
{
    fn drop(&mut self) {
        self.block.get_atomics(self.tag).incr_give();
    }
}
