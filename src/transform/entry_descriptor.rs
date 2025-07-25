use crate::transform::{
    block::Block,
    config::{FifoTag, IndexedDrop},
};

pub struct EntryDescriptor<
    'a,
    Tag: FifoTag,
    Inner: IndexedDrop<Tag> + Default,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> where
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
    pub(crate) block: &'a Block<Tag, Inner, BLOCK_SIZE, NUM_TRANSFORMATIONS>,
    pub(crate) index: usize,
    pub(crate) tag: Tag,
}

impl<
    'a,
    Tag: FifoTag,
    Inner: IndexedDrop<Tag> + Default,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> EntryDescriptor<'a, Tag, Inner, BLOCK_SIZE, NUM_TRANSFORMATIONS>
where
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
    pub fn modify_t_in_place<F: FnOnce(*mut Inner)>(&mut self, modifier: F) {
        modifier(unsafe { self.block.get_ptr(self.index) })
    }
}

impl<
    'a,
    Tag: FifoTag,
    Inner: IndexedDrop<Tag> + Default,
    const BLOCK_SIZE: usize,
    const NUM_TRANSFORMATIONS: usize,
> Drop for EntryDescriptor<'a, Tag, Inner, BLOCK_SIZE, NUM_TRANSFORMATIONS>
where
    [(); BLOCK_SIZE]:,
    [(); NUM_TRANSFORMATIONS]:,
{
    fn drop(&mut self) {
        self.block.get_atomics(self.tag).incr_give();
    }
}
