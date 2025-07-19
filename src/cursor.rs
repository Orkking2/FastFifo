use crate::util::greater_than_log_2;
use derive_more::{From, Into};
use std::{fmt::Debug, ops::Add};

#[derive(Default)]
pub struct CursorConfig {
    pub version: usize,
    pub offset: usize,
}

impl<const BLOCK_SIZE: usize> From<CursorConfig> for Cursor<BLOCK_SIZE> {
    fn from(value: CursorConfig) -> Self {
        let CursorConfig { offset, version } = value;

        let mut cursor = Cursor(0);
        cursor.set_offset(offset);
        cursor.set_version(version);
        cursor
    }
}

#[repr(transparent)]
#[derive(PartialEq, PartialOrd, Copy, Clone, From, Into)]
pub struct Cursor<const BLOCK_SIZE: usize>(usize);

impl<const BLOCK_SIZE: usize> Add<usize> for Cursor<BLOCK_SIZE> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl<const NUM_BLOCKS: usize> Default for Cursor<NUM_BLOCKS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const BLOCK_SIZE: usize> Cursor<BLOCK_SIZE> {
    pub const fn new() -> Self {
        Self(0)
    }

    pub const fn from_parts(version: usize, offset: usize) -> Self {
        Self((version << Self::version_shift()) | (offset & Self::offset_mask()))
    }

    const fn version_shift() -> u32 {
        greater_than_log_2(BLOCK_SIZE)
    }

    const fn offset_mask() -> usize {
        !(usize::MAX << Self::version_shift())
    }

    pub fn get_version(&self) -> usize {
        self.0 >> Self::version_shift()
    }

    pub fn set_version(&mut self, version: usize) {
        self.0 = self.get_offset() | (version << Self::version_shift())
    }

    pub fn get_offset(&self) -> usize {
        self.0 & Self::offset_mask()
    }

    pub fn set_offset(&mut self, offset: usize) {
        self.0 = self.get_version() | (offset & Self::offset_mask())
    }
}

impl<const BLOCK_SIZE: usize> Debug for Cursor<BLOCK_SIZE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cursor")
            .field("version", &self.get_version())
            .field("offset", &self.get_offset())
            .finish()
    }
}

// #[test]
// fn cursor_mask_print() {
//     const BLOCK_SIZE: usize = 7;

//     let mask = Cursor::<BLOCK_SIZE>::offset_mask();
//     println!("{mask:064b} offset mask");
//     let version_lshift = Cursor::<BLOCK_SIZE>::version_shift();
//     println!("{:064b} version mask", u64::MAX << version_lshift);

//     let x: Cursor<BLOCK_SIZE> = CursorConfig {
//         offset: 7,
//         version: 1,
//     }
//     .into();
//     println!("{:064b} raw of {x:?}", x.0);
// }
