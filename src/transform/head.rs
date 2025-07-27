use crate::{
    field::Field,
    transform::{config::FifoTag, wide_field::WideField},
};
use std::sync::atomic::{AtomicUsize, Ordering};

pub trait Atomic<const NUM_BLOCKS: usize, Tag: FifoTag> {
    fn load(&self) -> WideField<NUM_BLOCKS, Tag>;
    fn max(&self, rhs: Field<NUM_BLOCKS>) -> WideField<NUM_BLOCKS, Tag>;
}

/// For SP or SC
#[repr(C)]
pub struct NonAtomicHead<const NUM_BLOCKS: usize, Tag: FifoTag> {
    inner: Field<NUM_BLOCKS>,
    tag: Tag,
}

impl<const NUM_BLOCKS: usize, Tag: FifoTag> NonAtomicHead<NUM_BLOCKS, Tag> {
    pub const fn new(tag: Tag) -> Self {
        Self {
            inner: Field::new(),
            tag,
        }
    }

    pub const fn full(tag: Tag) -> Self {
        Self {
            inner: Field::full_minus_one(),
            tag,
        }
    }
}

impl<const NUM_BLOCKS: usize, Tag: FifoTag> Atomic<NUM_BLOCKS, Tag>
    for NonAtomicHead<NUM_BLOCKS, Tag>
{
    fn load(&self) -> WideField<NUM_BLOCKS, Tag> {
        WideField::from_parts(self.inner, self.tag.clone())
    }

    fn max(&self, rhs: Field<NUM_BLOCKS>) -> WideField<NUM_BLOCKS, Tag> {
        let old = self.load();
        if rhs > *old {
            // Safety: This layer was chosen to be non-atomic.
            unsafe {
                (&self.inner as *const Field<NUM_BLOCKS> as *mut Field<NUM_BLOCKS>).write(rhs);
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
