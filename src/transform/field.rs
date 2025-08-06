use std::{cmp::Ordering, fmt::Debug};

pub struct FieldConfig {
    pub index_max: usize,
    pub version: usize,
    pub index: usize,
}

#[derive(Clone, Copy)]
pub struct Field {
    index_max: usize,
    inner: usize,
}

impl From<FieldConfig> for Field {
    fn from(
        FieldConfig {
            index_max,
            version,
            index,
        }: FieldConfig,
    ) -> Self {
        Self::from_parts(index_max, version, index)
    }
}

impl Field {
    const fn version_shift(index_max: usize) -> u32 {
        usize::BITS - index_max.leading_zeros()
    }

    const fn index_mask(index_max: usize) -> usize {
        !(usize::MAX << Self::version_shift(index_max))
    }

    pub fn from_parts(index_max: usize, version: usize, index: usize) -> Self {
        Self {
            index_max,
            inner: (version << Self::version_shift(index_max))
                | (index & Self::index_mask(index_max)),
        }
    }

    pub fn from_raw_parts(index_max: usize, inner: usize) -> Self {
        Self { index_max, inner }
    }

    pub const fn get_version(&self) -> usize {
        self.inner >> Self::version_shift(self.index_max)
    }

    pub const fn get_index(&self) -> usize {
        self.inner & Self::index_mask(self.index_max)
    }

    pub fn get_index_max(&self) -> usize {
        self.index_max
    }

    pub fn get_raw_inner(&self) -> usize {
        self.inner
    }

    pub const fn overflowing_add(self, rhs: usize) -> Self {
        let Self { index_max, inner } = self;

        Self {
            index_max,
            inner: inner + rhs,
        }
    }

    pub fn version_inc_add(self, rhs: usize) -> Self {
        FieldConfig {
            index_max: self.index_max,
            version: self.get_version() + (self.get_index() + rhs) / self.index_max,
            index: (self.get_index() + rhs) % self.index_max,
        }
        .into()
    }
}

impl Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("index_max", &self.index_max)
            .field("version", &self.get_version())
            .field("index", &self.get_index())
            .finish()
    }
}

impl PartialOrd for Field {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

impl PartialEq for Field {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}
