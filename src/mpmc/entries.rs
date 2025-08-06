use super::block::Block;
use std::sync::atomic::Ordering;

/// Think of this as an allocator giving you exactly one *mut T.
pub struct ProducingEntry<'a, T, const BLOCK_SIZE: usize>(
    pub(crate) EntryDescription<'a, T, BLOCK_SIZE>,
);

impl<'a, T, const BLOCK_SIZE: usize> ProducingEntry<'a, T, BLOCK_SIZE> {
    pub fn produce_t_in_place<F: FnOnce(*mut T)>(&mut self, producer: F) {
        self.0.modify_t_in_place(producer);
    }
}

impl<'a, T, const BLOCK_SIZE: usize> Drop for ProducingEntry<'a, T, BLOCK_SIZE> {
    fn drop(&mut self) {
        // All subsequent reads must be visible after this increment.
        self.0.block.committed.fetch_add(1, Ordering::Release);
    }
}

/// Think of this as a deallocator, letting you do what needs to be done with *mut T before it gets freed.
#[repr(transparent)]
pub struct ConsumingEntry<'a, T, const BLOCK_SIZE: usize>(
    pub(crate) EntryDescription<'a, T, BLOCK_SIZE>,
);

impl<'a, T, const BLOCK_SIZE: usize> ConsumingEntry<'a, T, BLOCK_SIZE> {
    pub fn consume_t_in_place<F: FnOnce(*mut T)>(&mut self, consumer: F) {
        self.0.modify_t_in_place(consumer);
    }
}

impl<'a, T, const BLOCK_SIZE: usize> Drop for ConsumingEntry<'a, T, BLOCK_SIZE> {
    fn drop(&mut self) {
        self.0.block.consumed.fetch_add(1, Ordering::Release);
    }
}

pub(crate) struct EntryDescription<'a, T, const BLOCK_SIZE: usize> {
    pub(crate) block: &'a Block<T, BLOCK_SIZE>,
    pub(crate) index: usize,
    #[allow(dead_code)]
    pub(crate) version: usize,
}

impl<'a, T, const BLOCK_SIZE: usize> EntryDescription<'a, T, BLOCK_SIZE> {
    /// Modify *mut T in-place
    pub fn modify_t_in_place<F: FnOnce(*mut T)>(&mut self, modifier: F) {
        modifier(self.block.entries[self.index].as_ptr() as *mut T)
    }
}
