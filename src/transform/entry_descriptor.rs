use crate::transform::{block::Block, config::FifoConfig};

pub struct EntryDescriptor<'a, Config: FifoConfig>
where
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    pub(crate) block: &'a Block<Config>,
    pub(crate) index: usize,
    pub(crate) tag: <Config as FifoConfig>::Tag,
}

impl<'a, Config: FifoConfig> EntryDescriptor<'a, Config>
where
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    pub fn modify_t_in_place<F: FnOnce(*mut <Config as FifoConfig>::Inner)>(
        &mut self,
        modifier: F,
    ) {
        modifier(unsafe { self.block.get_ptr(self.index) })
    }
}

impl<'a, Config: FifoConfig> Drop for EntryDescriptor<'a, Config>
where
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    fn drop(&mut self) {
        

        self.block.get_atomics(self.tag).incr_give();

        
    }
}
