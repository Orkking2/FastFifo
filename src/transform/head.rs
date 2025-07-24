use crate::{
    field::Field,
    transform::{config::FifoTag, wide_field::WideField},
};
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

pub trait Atomic<const NUM_BLOCKS: usize, Tag: FifoTag> {
    fn load(&self) -> WideField<NUM_BLOCKS, Tag>;
    fn max(&self, rhs: Field<NUM_BLOCKS>) -> WideField<NUM_BLOCKS, Tag>;
}

/// For SP or SC
#[repr(C)]
pub struct NonAtomicHead<const NUM_BLOCKS: usize, Tag: FifoTag> {
    inner: UnsafeCell<Field<NUM_BLOCKS>>,
    tag: Tag,
}

impl<const NUM_BLOCKS: usize, Tag: FifoTag> NonAtomicHead<NUM_BLOCKS, Tag> {
    pub const fn new(tag: Tag) -> Self {
        Self {
            inner: UnsafeCell::new(Field::new()),
            tag,
        }
    }

    pub const fn full(tag: Tag) -> Self {
        Self {
            inner: UnsafeCell::new(Field::full_minus_one()),
            tag,
        }
    }
}

impl<const NUM_BLOCKS: usize, Tag: FifoTag> Atomic<NUM_BLOCKS, Tag>
    for NonAtomicHead<NUM_BLOCKS, Tag>
{
    fn load(&self) -> WideField<NUM_BLOCKS, Tag> {
        WideField::from_parts(*unsafe { self.inner.as_ref_unchecked() }, self.tag.clone())
    }

    fn max(&self, rhs: Field<NUM_BLOCKS>) -> WideField<NUM_BLOCKS, Tag> {
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
pub struct AtomicHead<const NUM_BLOCKS: usize, Tag: FifoTag> {
    inner: AtomicUsize,
    tag: Tag,
}

impl<const NUM_BLOCKS: usize, Tag: FifoTag> AtomicHead<NUM_BLOCKS, Tag> {
    pub fn new(tag: Tag) -> Self {
        Self {
            inner: AtomicUsize::new(Field::<NUM_BLOCKS>::new().into()),
            tag,
        }
    }

    pub fn full(tag: Tag) -> Self {
        Self {
            inner: AtomicUsize::new(Field::<NUM_BLOCKS>::full_minus_one().into()),
            tag,
        }
    }
}

impl<const NUM_BLOCKS: usize, Tag: FifoTag> Atomic<NUM_BLOCKS, Tag>
    for AtomicHead<NUM_BLOCKS, Tag>
{
    fn load(&self) -> WideField<NUM_BLOCKS, Tag> {
        WideField::from_parts(
            Field::from(self.inner.load(Ordering::Relaxed)),
            self.tag.clone(),
        )
    }

    fn max(&self, rhs: Field<NUM_BLOCKS>) -> WideField<NUM_BLOCKS, Tag> {
        WideField::from_parts(
            Field::from(self.inner.fetch_max(rhs.into(), Ordering::Relaxed)),
            self.tag.clone(),
        )
    }
}
