use crate::util::ceiling_log_2;
use derive_more::{From, Into};
use std::{fmt::Debug, ops::Add};

pub struct HeadConfig {
    pub version: usize,
    pub index: usize,
}

impl<const NUM_BLOCKS: usize> From<HeadConfig> for Head<NUM_BLOCKS> {
    fn from(value: HeadConfig) -> Self {
        let HeadConfig { version, index } = value;

        Head::from_parts(version, index)
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, From, Into)]
pub struct Head<const NUM_BLOCKS: usize>(usize);

// BLOCK_NUM (NUM_BLOCKS) = 100 -> log_2(100) = 6.64 ~ 7 (7 bits needed to store index within 100)
// 0b0000_0000_0000_0000_0000...0 | 000_0000
// 0b1111_1111_1111_1111_1111...1 | 111_1111 (left shift by 7 to retrieve version (highest bits))

impl<const NUM_BLOCKS: usize> Default for Head<NUM_BLOCKS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const NUM_BLOCKS: usize> Head<NUM_BLOCKS> {
    pub const fn new() -> Self {
        Self(0)
    }

    pub const fn from_parts(version: usize, index: usize) -> Self {
        Self((version << Self::version_shift()) | (index & Self::index_mask()))
    }

    const fn version_shift() -> u32 {
        ceiling_log_2(NUM_BLOCKS)
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
}

impl<const NUM_BLOCKS: usize> Add<usize> for Head<NUM_BLOCKS> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        // If adding rhs to index would overflow, add it to version instead
        // Using /, % avoids needing a loop
        Head::from(HeadConfig {
            version: self.get_version() + (self.get_index() + rhs) / NUM_BLOCKS,
            index: (self.get_index() + rhs) % NUM_BLOCKS,
        })
    }
}

impl<const NUM_BLOCKS: usize> Debug for Head<NUM_BLOCKS> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Head")
            .field("version", &self.get_version())
            .field("index", &self.get_index())
            .finish()
    }
}

#[test]
fn head_mask_print() {
    const NUM_BLOCKS: usize = 2;

    let mask = Head::<NUM_BLOCKS>::index_mask();
    println!("{mask:064b} index mask");
    let version_lshift = Head::<NUM_BLOCKS>::version_shift();
    println!("{:064b} version mask", u64::MAX << version_lshift);

    let x: Head<NUM_BLOCKS> = HeadConfig {
        index: 7,
        version: 4096 - 1,
    }
    .into();
    println!("{:064b} raw of {x:?}", x.0);
}
