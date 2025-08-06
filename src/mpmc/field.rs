use derive_more::{From, Into};
use std::fmt::Debug;

pub struct FieldConfig {
    pub version: usize,
    pub index: usize,
}

impl Default for FieldConfig {
    /// Unless specified, {.vsn = 0, .idx = 0}
    fn default() -> Self {
        Self {
            version: 0,
            index: 0,
        }
    }
}

impl<const INDEX_MAX: usize> From<FieldConfig> for Field<INDEX_MAX> {
    fn from(value: FieldConfig) -> Self {
        let FieldConfig { version, index } = value;

        Field::from_parts(version, index)
    }
}

#[derive(PartialEq, PartialOrd, Copy, Clone, From, Into)]
pub struct Field<const INDEX_MAX: usize>(usize);

// BLOCK_NUM (INDEX_MAX) = 100 -> log_2(100) = 6.64 ~ 7 (7 bits needed to store index within 100)
// 0b0000_0000_0000_0000_0000...0 | 000_0000
// 0b1111_1111_1111_1111_1111...1 | 111_1111 (left shift by 7 to retrieve version (highest bits))

impl<const INDEX_MAX: usize> Default for Field<INDEX_MAX> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const INDEX_MAX: usize> Field<INDEX_MAX> {
    pub const fn new() -> Self {
        Self(0)
    }

    pub const fn full() -> Self {
        Self(INDEX_MAX)
    }

    pub const fn full_minus_one() -> Self {
        Self(INDEX_MAX - 1)
    }

    pub const fn from_parts(version: usize, index: usize) -> Self {
        Self((version << Self::version_shift()) | (index & Self::index_mask()))
    }

    const fn version_shift() -> u32 {
        // log_2 (INDEX_MAX)
        usize::BITS - INDEX_MAX.leading_zeros()
        // 32
    }

    const fn index_mask() -> usize {
        !(usize::MAX << Self::version_shift())
    }

    pub const fn get_version(&self) -> usize {
        self.0 >> Self::version_shift()
    }

    pub const fn set_version(&mut self, version: usize) {
        self.0 = self.get_index() | (version << Self::version_shift())
    }

    pub const fn get_index(&self) -> usize {
        self.0 & Self::index_mask()
    }

    pub const fn set_index(&mut self, index: usize) {
        self.0 = self.get_version() | (index & Self::index_mask())
    }

    pub const fn overflowing_add(self, rhs: usize) -> Self {
        Self(self.0 + rhs)
    }

    pub fn version_inc_add(self, rhs: usize) -> Self {
        Self::from(FieldConfig {
            version: self.get_version() + (self.get_index() + rhs) / INDEX_MAX,
            index: (self.get_index() + rhs) % INDEX_MAX,
        })
    }
}

impl<const INDEX_MAX: usize> Debug for Field<INDEX_MAX> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("version", &self.get_version())
            .field("index", &self.get_index())
            .finish()
    }
}

// #[test]
// fn Field_mask_print() {
//     const INDEX_MAX: usize = 2;

//     let mask = Field::<INDEX_MAX>::index_mask();
//
//     let version_lshift = Field::<INDEX_MAX>::version_shift();
//

//     let x: Field<INDEX_MAX> = FieldConfig {
//         index: 7,
//         version: 4096 - 1,
//     }
//     .into();
//
// }
