use crate::{field::Field, transform::config::FifoTag};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy)]
pub struct WideField<const INDEX_MAX: usize, Tag: FifoTag> {
    inner: Field<INDEX_MAX>,
    tag: Tag,
}

impl<const INDEX_MAX: usize, Tag: FifoTag> Deref for WideField<INDEX_MAX, Tag> {
    type Target = Field<INDEX_MAX>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<const INDEX_MAX: usize, Tag: FifoTag> DerefMut for WideField<INDEX_MAX, Tag> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<const INDEX_MAX: usize, Tag: FifoTag> WideField<INDEX_MAX, Tag> {
    pub const fn from_parts(field: Field<INDEX_MAX>, tag: Tag) -> Self {
        Self { inner: field, tag }
    }

    pub fn get_tag(&self) -> Tag {
        self.tag.clone()
    }
}
