use crate::{
    field::Field,
    transform::{layer::Layer, wide_field::WideField},
};
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

pub trait Atomic<const NUM_BLOCKS: usize> {
    fn load(&self) -> WideField<NUM_BLOCKS>;
    fn max(&self, rhs: Field<NUM_BLOCKS>) -> WideField<NUM_BLOCKS>;
}

/// For SP or SC
#[repr(C)]
pub struct NonAtomicHead<const NUM_BLOCKS: usize> {
    inner: UnsafeCell<Field<NUM_BLOCKS>>,
    layer: Layer,
}

impl<const NUM_BLOCKS: usize> NonAtomicHead<NUM_BLOCKS> {
    pub const fn new(layer: Layer) -> Self {
        Self {
            inner: UnsafeCell::new(Field::new()),
            layer,
        }
    }
}

impl<const NUM_BLOCKS: usize> Atomic<NUM_BLOCKS> for NonAtomicHead<NUM_BLOCKS> {
    fn load(&self) -> WideField<NUM_BLOCKS> {
        WideField::from_parts(*unsafe { self.inner.as_ref_unchecked() }, self.layer)
    }

    fn max(&self, rhs: Field<NUM_BLOCKS>) -> WideField<NUM_BLOCKS> {
        let old = self.load();
        if rhs > *old {
            unsafe {
                self.inner.replace(rhs);
            }
        }
        old
    }
}

/// For MP or MC
#[repr(C)]
pub struct AtomicHead<const NUM_BLOCKS: usize> {
    inner: AtomicUsize,
    layer: Layer,
}

impl<const NUM_BLOCKS: usize> AtomicHead<NUM_BLOCKS> {
    pub fn new(layer: Layer) -> Self {
        Self {
            inner: AtomicUsize::new(Field::<NUM_BLOCKS>::new().into()),
            layer,
        }
    }
}

impl<const NUM_BLOCKS: usize> Atomic<NUM_BLOCKS> for AtomicHead<NUM_BLOCKS> {
    fn load(&self) -> WideField<NUM_BLOCKS> {
        WideField::from_parts(Field::from(self.inner.load(Ordering::Relaxed)), self.layer)
    }

    fn max(&self, rhs: Field<NUM_BLOCKS>) -> WideField<NUM_BLOCKS> {
        WideField::from_parts(
            Field::from(self.inner.fetch_max(rhs.into(), Ordering::Relaxed)),
            self.layer,
        )
    }
}
