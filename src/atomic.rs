use crate::field::Field;
use std::{
    fmt::Debug,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct AtomicField<const INDEX_MAX: usize>(AtomicUsize);

impl<const INDEX_MAX: usize> AtomicField<INDEX_MAX> {
    pub fn new<U: Into<Field<INDEX_MAX>>>(value: U) -> Self {
        Self(AtomicUsize::new(value.into().into()))
    }

    pub fn load(&self, order: Ordering) -> Field<INDEX_MAX> {
        Field::from(self.0.load(order))
    }

    pub fn fetch_add(&self, val: usize, order: Ordering) -> Field<INDEX_MAX> {
        Field::from(self.0.fetch_add(val, order))
    }

    pub fn fetch_max(&self, val: Field<INDEX_MAX>, order: Ordering) -> Field<INDEX_MAX> {
        Field::from(self.0.fetch_max(val.into(), order))
    }
}

impl<const INDEX_MAX: usize> Default for AtomicField<INDEX_MAX> {
    fn default() -> Self {
        Self {
            0: Default::default(),
        }
    }
}

impl<const INDEX_MAX: usize> Debug for AtomicField<INDEX_MAX> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Atomic")
            .field("0", &self.load(Ordering::Relaxed))
            .finish()
    }
}
