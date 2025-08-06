use crate::field::Field;
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

pub trait Atomic {
    fn load(&self) -> Field;
    fn max(&self, rhs: Field) -> Field;
}

/// For SP or SC
pub struct NonAtomicHead(UnsafeCell<Field>);

impl From<Field> for NonAtomicHead {
    fn from(value: Field) -> Self {
        Self(UnsafeCell::new(value))
    }
}

impl Atomic for NonAtomicHead {
    fn load(&self) -> Field {
        unsafe { self.0.get().read() }
    }

    fn max(&self, rhs: Field) -> Field {
        let old = self.load();
        if rhs > old {
            // Safety: This layer was chosen to be non-atomic.
            unsafe {
                self.0.get().write(rhs);
            }
            old
        } else {
            rhs
        }
    }
}

/// For MP or MC
#[repr(C)]
pub struct AtomicHead {
    index_max: usize,
    inner: AtomicUsize,
}

impl From<Field> for AtomicHead {
    fn from(value: Field) -> Self {
        Self {
            index_max: value.get_index_max(),
            inner: AtomicUsize::new(value.get_raw_inner()),
        }
    }
}

impl Atomic for AtomicHead {
    fn load(&self) -> Field {
        Field::from_raw_parts(self.index_max, self.inner.load(Ordering::Relaxed))
    }

    fn max(&self, rhs: Field) -> Field {
        Field::from_raw_parts(
            self.index_max,
            self.inner.fetch_max(rhs.get_raw_inner(), Ordering::Relaxed),
        )
    }
}
