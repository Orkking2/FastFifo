use crate::transform::{config::FifoTag, field::Field, wide_field::WideField};
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

pub trait Atomic<Tag: FifoTag> {
    fn load(&self) -> WideField<Tag>;
    fn max(&self, rhs: Field) -> WideField<Tag>;
}

/// For SP or SC
pub struct NonAtomicHead<Tag: FifoTag>(UnsafeCell<WideField<Tag>>);

impl<Tag: FifoTag> NonAtomicHead<Tag> {
    pub fn from_parts(index_max: usize, version: usize, index: usize, tag: Tag) -> Self {
        Self(UnsafeCell::new(WideField::from_parts(
            Field::from_parts(index_max, version, index),
            tag,
        )))
    }
}

impl<Tag: FifoTag> From<WideField<Tag>> for NonAtomicHead<Tag> {
    fn from(value: WideField<Tag>) -> Self {
        Self(UnsafeCell::new(value))
    }
}

impl<Tag: FifoTag> Atomic<Tag> for NonAtomicHead<Tag> {
    fn load(&self) -> WideField<Tag> {
        unsafe { self.0.get().read() }
    }

    fn max(&self, rhs: Field) -> WideField<Tag> {
        let old = self.load();
        if rhs > *old {
            // Safety: This layer was chosen to be non-atomic.
            unsafe {
                self.0
                    .get()
                    .write(WideField::from_parts(rhs, old.get_tag()));
            }
            old
        } else {
            WideField::from_parts(rhs, old.get_tag())
        }
    }
}

/// For MP or MC
#[repr(C)]
pub struct AtomicHead<Tag: FifoTag> {
    index_max: usize,
    inner: AtomicUsize,
    tag: Tag,
}

impl<Tag: FifoTag> AtomicHead<Tag> {
    pub fn from_parts(index_max: usize, version: usize, index: usize, tag: Tag) -> Self {
        Self {
            index_max,
            inner: AtomicUsize::new(Field::from_parts(index_max, version, index).get_raw_inner()),
            tag,
        }
    }
}

impl<Tag: FifoTag> From<WideField<Tag>> for AtomicHead<Tag> {
    fn from(value: WideField<Tag>) -> Self {
        Self {
            index_max: value.get_index_max(),
            inner: AtomicUsize::new(value.get_raw_inner()),
            tag: value.get_tag(),
        }
    }
}

impl<Tag: FifoTag> Atomic<Tag> for AtomicHead<Tag> {
    fn load(&self) -> WideField<Tag> {
        WideField::from_parts(
            Field::from_raw_parts(self.index_max, self.inner.load(Ordering::Relaxed)),
            self.tag.clone(),
        )
    }

    fn max(&self, rhs: Field) -> WideField<Tag> {
        WideField::from_parts(
            Field::from_raw_parts(
                self.index_max,
                self.inner.fetch_max(rhs.get_raw_inner(), Ordering::Relaxed),
            ),
            self.tag.clone(),
        )
    }
}
