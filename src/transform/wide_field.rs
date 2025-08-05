use crate::transform::{config::FifoTag, field::Field};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug)]
pub struct WideField<Tag: FifoTag> {
    inner: Field,
    tag: Tag,
}

impl<Tag: FifoTag> Deref for WideField<Tag> {
    type Target = Field;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<Tag: FifoTag> DerefMut for WideField<Tag> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<Tag: FifoTag> WideField<Tag> {
    pub const fn from_parts(field: Field, tag: Tag) -> Self {
        Self { inner: field, tag }
    }

    pub fn get_tag(&self) -> Tag {
        self.tag.clone()
    }
}
