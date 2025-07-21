use derive_more::{From, Into};
use std::{fmt::Debug, ops::Add};

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

impl<const NUM_BLOCKS: usize> From<FieldConfig> for Field<NUM_BLOCKS> {
    fn from(value: FieldConfig) -> Self {
        let FieldConfig { version, index } = value;

        Field::from_parts(version, index)
    }
}

#[derive(PartialEq, Copy, Clone, From, Into)]
pub struct Field<const NUM_BLOCKS: usize>(usize);

// BLOCK_NUM (NUM_BLOCKS) = 100 -> log_2(100) = 6.64 ~ 7 (7 bits needed to store index within 100)
// 0b0000_0000_0000_0000_0000...0 | 000_0000
// 0b1111_1111_1111_1111_1111...1 | 111_1111 (left shift by 7 to retrieve version (highest bits))

impl<const NUM_BLOCKS: usize> Default for Field<NUM_BLOCKS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const NUM_BLOCKS: usize> Field<NUM_BLOCKS> {
    pub const fn new() -> Self {
        Self(0)
    }

    pub const fn from_parts(version: usize, index: usize) -> Self {
        Self((version << Self::version_shift()) | (index & Self::index_mask()))
    }

    const fn version_shift() -> u32 {
        // log_2 (NUM_BLOCKS)
        usize::BITS - NUM_BLOCKS.leading_zeros()
    }

    const fn index_mask() -> usize {
        !(usize::MAX << Self::version_shift())
    }

    pub fn get_version(&self) -> usize {
        self.0 >> Self::version_shift()
    }

    pub fn set_version(&mut self, version: usize) {
        self.0 = self.get_index() | (version << Self::version_shift())
    }

    pub fn get_index(&self) -> usize {
        self.0 & Self::index_mask()
    }

    pub fn set_index(&mut self, index: usize) {
        self.0 = self.get_version() | (index & Self::index_mask())
    }

    pub fn overflowing_add(self, rhs: usize) -> Self {
        Self(self.0 + rhs)
    }

    pub fn version_inc_add(self, rhs: usize) -> Self {
        Self::from(FieldConfig {
            version: self.get_version() + (self.get_index() + rhs) / NUM_BLOCKS,
            index: (self.get_index() + rhs) % NUM_BLOCKS,
        })
    }
}

impl<const NUM_BLOCKS: usize> Debug for Field<NUM_BLOCKS> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("version", &self.get_version())
            .field("index", &self.get_index())
            .finish()
    }
}

// #[test]
// fn Field_mask_print() {
//     const NUM_BLOCKS: usize = 2;

//     let mask = Field::<NUM_BLOCKS>::index_mask();
//     println!("{mask:064b} index mask");
//     let version_lshift = Field::<NUM_BLOCKS>::version_shift();
//     println!("{:064b} version mask", u64::MAX << version_lshift);

//     let x: Field<NUM_BLOCKS> = FieldConfig {
//         index: 7,
//         version: 4096 - 1,
//     }
//     .into();
//     println!("{:064b} raw of {x:?}", x.0);
// }
