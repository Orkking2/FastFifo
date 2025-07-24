use std::sync::atomic::{AtomicUsize, Ordering};

use crate::field::Field;

#[repr(C)]
pub struct AtomicPair<const BLOCK_SIZE: usize> {
    take: AtomicUsize,
    give: AtomicUsize,
}

impl<const BLOCK_SIZE: usize> Default for AtomicPair<BLOCK_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const BLOCK_SIZE: usize> AtomicPair<BLOCK_SIZE> {
    pub fn new() -> Self {
        Self {
            take: AtomicUsize::new(0),
            give: AtomicUsize::new(0),
        }
    }

    pub fn full() -> Self {
        Self {
            take: AtomicUsize::new(BLOCK_SIZE),
            give: AtomicUsize::new(BLOCK_SIZE),
        }
    }

    pub fn load_take(&self) -> Field<BLOCK_SIZE> {
        Field::from(self.take.load(Ordering::Relaxed))
    }

    // pub fn incr_take(&self) {
    //     self.take.fetch_add(1, Ordering::Relaxed);
    // }

    pub fn fetch_max_take(&self, val: Field<BLOCK_SIZE>) -> Field<BLOCK_SIZE> {
        Field::from(self.take.fetch_max(val.into(), Ordering::Relaxed))
    }

    /// Must be aquire so previous give stores are seen before this one is loaded
    pub fn load_give(&self) -> Field<BLOCK_SIZE> {
        Field::from(self.give.load(Ordering::Acquire))
    }

    /// Must be release so subsequent give loads are seen after this one is stored
    pub fn incr_give(&self) {
        self.give.fetch_add(1, Ordering::Release);
    }

    pub fn fetch_max_give(&self, val: Field<BLOCK_SIZE>) -> Field<BLOCK_SIZE> {
        Field::from(self.give.fetch_max(val.into(), Ordering::Relaxed))
    }

    /// Returns old (give, take)
    pub fn fetch_max_both<T: Into<Field<BLOCK_SIZE>>>(
        &self,
        val: T,
    ) -> (Field<BLOCK_SIZE>, Field<BLOCK_SIZE>) {
        let val = val.into();

        (self.fetch_max_give(val), self.fetch_max_take(val))
    }
}
