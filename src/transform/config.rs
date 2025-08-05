use std::fmt::Debug;

// pub trait FifoConfig {
//     type Tag: FifoTag;

//     type Inner: IndexedDrop + Default;

//     const NUM_TRANSFORMATIONS: usize;
//     const NUM_BLOCKS: usize;
//     const BLOCK_SIZE: usize;
// }

pub trait TaggedClone<Tag: FifoTag>: Sized {
    fn tagged_clone(&self, tag: Tag) -> Option<Self> {
        if tag.is_atomic() {
            Some(self.unchecked_clone())
        } else {
            None
        }
    }

    fn unchecked_clone(&self) -> Self;
}

pub trait IndexedDrop<Tag: FifoTag> {
    /// # Safety
    /// Calling this method anywhere outside of a `Drop` impl is undefined behaviour.
    unsafe fn tagged_drop(&mut self, tag: Tag);

    /// Attempts to convert an `index` to a `Tag` and call `tagged_drop`. If this fails,
    /// we assume the memory is uninitialized and simply refrain from calling drop on it.
    /// Theoretically there could be several stages of uninitialized memory?
    ///
    /// The default implementation is a simple forwarding of `drop`, specifically `std::ptr::drop_in_place`.
    unsafe fn indexed_drop(&mut self, index: usize) {
        if let Ok(tag) = Tag::try_from(index) {
            unsafe { self.tagged_drop(tag) }
        }
    }
}

pub trait FifoTag: TryFrom<usize, Error: Debug> + Into<usize> + Copy {
    fn is_atomic(self) -> bool;
    fn chases(self) -> Self;

    fn producer() -> Self;
    /// It is expected that every element in 0..Tag::num_transformations() can be converted to a Tag.
    /// 
    /// Make sure to implement a Self::try_from(Tag::num_transformations()) for custom drop behaviour.
    fn num_transformations() -> usize;
}
