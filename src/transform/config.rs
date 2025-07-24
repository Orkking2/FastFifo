use std::{fmt::Debug, ptr};

pub trait FifoConfig {
    type Tag: FifoTag;

    type Inner: IndexedDrop + Default;

    const NUM_TRANSFORMATIONS: usize;
    const NUM_BLOCKS: usize;
    const BLOCK_SIZE: usize;
}

pub trait TaggedClone: Sized {
    type Tag: FifoTag;

    fn tagged_clone(&self, tag: Self::Tag) -> Option<Self> {
        if tag.is_atomic() {
            Some(self.unchecked_clone())
        } else {
            None
        }
    }

    fn unchecked_clone(&self) -> Self;
}

pub trait IndexedDrop {
    type Tag: TryFrom<usize> + FifoTag;

    /// # Safety
    /// Calling this method anywhere outside of a `Drop` impl is undefined behaviour.
    unsafe fn tagged_drop(&mut self, tag: Self::Tag);

    /// Attempts to convert an `index` to a `Tag` and call `tagged_drop`. If this fails,
    /// we assume the memory is uninitialized and simply refrain from calling drop on it.
    /// Theoretically there could be several stages of uninitialized memory?
    ///
    /// The default implementation is a simple forwarding of `drop`, specifically `std::ptr::drop_in_place`.
    unsafe fn indexed_drop(&mut self, index: usize) {
        if let Ok(tag) = Self::Tag::try_from(index) {
            unsafe { self.tagged_drop(tag) }
        }
    }
}

pub trait FifoTag: TryFrom<usize, Error: Debug> + Into<usize> + Copy {
    fn is_atomic(self) -> bool;
    fn chases(self) -> Self;

    fn is_consumer(self, consumer_index: usize) -> bool {
        self.into() == consumer_index
    }
}
