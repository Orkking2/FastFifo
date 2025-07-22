use crate::{field::Field, transform::layer::Layer};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy)]
pub struct WideField<const INDEX_MAX: usize> {
    inner: Field<INDEX_MAX>,
    layer: Layer,
}

impl<const INDEX_MAX: usize> Deref for WideField<INDEX_MAX> {
    type Target = Field<INDEX_MAX>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<const INDEX_MAX: usize> DerefMut for WideField<INDEX_MAX> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<const INDEX_MAX: usize> WideField<INDEX_MAX> {
    pub const fn new(layer: Layer) -> Self {
        Self {
            inner: Field::new(),
            layer,
        }
    }

    pub const fn from_parts(field: Field<INDEX_MAX>, layer: Layer) -> Self {
        Self { inner: field, layer }
    }

    pub const fn get_layer(&self) -> Layer {
        self.layer
    }
}
