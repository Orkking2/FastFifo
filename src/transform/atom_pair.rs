use crate::transform::field::Field;
use std::sync::atomic::{AtomicUsize, Ordering};

#[repr(C)]
pub struct AtomicPair {
    index_max: usize,
    take: AtomicUsize,
    give: AtomicUsize,
}

impl From<Field> for AtomicPair {
    fn from(value: Field) -> Self {
        Self::from_raw_parts(value.get_index_max(), value.get_raw_inner())
    }
}

impl AtomicPair {
    pub fn from_raw_parts(index_max: usize, value: usize) -> Self {
        Self {
            index_max,
            take: AtomicUsize::new(value),
            give: AtomicUsize::new(value),
        }
    }

    pub fn load_take(&self) -> Field {
        Field::from_raw_parts(self.index_max, self.take.load(Ordering::Relaxed))
    }

    // pub fn incr_take(&self) {
    //     self.take.fetch_add(1, Ordering::Relaxed);
    // }

    pub fn fetch_max_take(&self, val: Field) -> Field {
        Field::from_raw_parts(
            self.index_max,
            self.take.fetch_max(val.get_raw_inner(), Ordering::Relaxed),
        )
    }

    /// Must be aquire so previous give stores are seen before this one is loaded
    pub fn load_give(&self) -> Field {
        Field::from_raw_parts(self.index_max, self.give.load(Ordering::Acquire))
    }

    /// Must be release so subsequent give loads are seen after this one is stored
    pub fn incr_give(&self) {
        self.give.fetch_add(1, Ordering::Release);
    }

    pub fn fetch_max_give(&self, val: Field) -> Field {
        Field::from_raw_parts(
            self.index_max,
            self.give.fetch_max(val.get_raw_inner(), Ordering::Relaxed),
        )
    }

    /// Returns old (give, take)
    pub fn fetch_max_both(&self, val: Field) -> (Field, Field) {
        (self.fetch_max_give(val), self.fetch_max_take(val))
    }
}
