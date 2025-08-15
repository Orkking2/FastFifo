use crate::field::Field;

#[cfg(not(loom))]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(loom)]
use loom::sync::atomic::{AtomicUsize, Ordering};

#[repr(C)]
pub struct AtomicPair {
    index_max: usize,
    take: AtomicUsize,
    give: AtomicUsize,
}

impl From<Field> for AtomicPair {
    fn from(value: Field) -> Self {
        Self::from_raw_parts(value.get_index_max(), value.get_raw_inner())
    }
}

impl AtomicPair {
    pub fn from_raw_parts(index_max: usize, inner: usize) -> Self {
        Self {
            index_max,
            take: AtomicUsize::new(inner),
            give: AtomicUsize::new(inner),
        }
    }

    pub fn load_take(&self) -> Field {
        Field::from_raw_parts(self.index_max, self.take.load(Ordering::Relaxed))
    }

    pub fn fetch_max_take(&self, val: Field) -> Field {
        Field::from_raw_parts(
            self.index_max,
            self.take.fetch_max(val.get_raw_inner(), Ordering::Relaxed),
        )
    }

    /// Must be aquire so previous give stores are seen before this one is loaded
    pub fn load_give(&self) -> Field {
        Field::from_raw_parts(self.index_max, self.give.load(Ordering::Acquire))
    }

    /// Must be release so subsequent give loads are seen after this one is stored
    pub fn incr_give(&self) {
        self.give.fetch_add(1, Ordering::Release);
    }

    pub fn fetch_max_give(&self, val: Field) -> Field {
        Field::from_raw_parts(
            self.index_max,
            self.give.fetch_max(val.get_raw_inner(), Ordering::Relaxed),
        )
    }

    /// Returns old (give, take)
    pub fn fetch_max_both(&self, val: Field) -> (Field, Field) {
        (self.fetch_max_give(val), self.fetch_max_take(val))
    }
}

#[cfg(test)]
mod test {
    use super::AtomicPair;
    use crate::field::Field;
    use rand::{
        Rng,
        distr::{Distribution, StandardUniform},
        rng,
    };
    // use std::{fmt::Debug, sync::atomic::Ordering};

    struct AtomicPairParts {
        index_max: usize,
        inner: usize,
    }

    impl Distribution<AtomicPairParts> for StandardUniform {
        fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> AtomicPairParts {
            AtomicPairParts {
                index_max: rng.random::<u64>() as usize,
                inner: rng.random::<u64>() as usize,
            }
        }
    }

    impl AtomicPairParts {
        fn get_atom_pair(&self) -> AtomicPair {
            AtomicPair::from_raw_parts(self.index_max, self.inner)
        }
    }

    // impl Debug for AtomicPair {
    //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //         f.debug_struct("AtomicPair")
    //             .field("index_max", &self.index_max)
    //             .field("give", &self.give.load(Ordering::Relaxed))
    //             .field("take", &self.take.load(Ordering::Relaxed))
    //             .finish()
    //     }
    // }

    // impl PartialEq for AtomicPair {
    //     fn eq(&self, other: &Self) -> bool {
    //         self.index_max == other.index_max
    //             && self.take.load(Ordering::Relaxed) == other.take.load(Ordering::Relaxed)
    //             && self.give.load(Ordering::Relaxed) == other.give.load(Ordering::Relaxed)
    //     }
    // }

    #[test]
    fn test_from_raw_parts_and_load() {
        let mut rng = rng();

        for _ in 0..1000 {
            let pair_parts = rng.random::<AtomicPairParts>();
            let pair = pair_parts.get_atom_pair();
            assert_eq!(pair.index_max, pair_parts.index_max);
            assert_eq!(pair.load_take().get_raw_inner(), pair_parts.inner);
            assert_eq!(pair.load_give().get_raw_inner(), pair_parts.inner);
        }
    }

    #[test]
    fn test_incr_give() {
        let mut rng = rng();

        for _ in 0..1000 {
            let pair_parts = rng.random::<AtomicPairParts>();
            let pair = pair_parts.get_atom_pair();
            let before = pair.load_give().get_raw_inner();
            pair.incr_give();
            let after = pair.load_give().get_raw_inner();
            assert_eq!(after, before + 1);
        }
    }

    #[test]
    fn test_fetch_max_take() {
        let mut rng = rng();

        for _ in 0..1000 {
            let pair_parts = rng.random::<AtomicPairParts>();
            let pair = pair_parts.get_atom_pair();
            let off = rng.random::<u64>() as usize % (usize::MAX - pair_parts.inner);
            let field = Field::from_raw_parts(pair_parts.index_max, pair_parts.inner + off);
            let old = pair.fetch_max_take(field);
            assert_eq!(old.get_raw_inner(), pair_parts.inner);
            assert_eq!(pair.load_take().get_raw_inner(), pair_parts.inner + off);
        }
    }

    #[test]
    fn test_fetch_max_give() {
        let mut rng = rng();

        for _ in 0..1000 {
            let pair_parts = rng.random::<AtomicPairParts>();
            let pair = pair_parts.get_atom_pair();
            let off = rng.random::<u64>() as usize % (usize::MAX - pair_parts.inner);
            let field = Field::from_raw_parts(pair_parts.index_max, pair_parts.inner + off);
            let old = pair.fetch_max_give(field);
            assert_eq!(old.get_raw_inner(), pair_parts.inner);
            assert_eq!(pair.load_give().get_raw_inner(), pair_parts.inner + off);
        }
    }

    #[test]
    fn test_fetch_max_both() {
        let mut rng = rng();

        for _ in 0..1000 {
            let pair_parts = rng.random::<AtomicPairParts>();
            let pair = pair_parts.get_atom_pair();
            let off = rng.random::<u64>() as usize % (usize::MAX - pair_parts.inner);
            let field = Field::from_raw_parts(pair_parts.index_max, pair_parts.inner + off);
            let (old_give, old_take) = pair.fetch_max_both(field);
            assert_eq!(old_give.get_raw_inner(), pair_parts.inner);
            assert_eq!(old_take.get_raw_inner(), pair_parts.inner);
            assert_eq!(pair.load_give().get_raw_inner(), pair_parts.inner + off);
            assert_eq!(pair.load_take().get_raw_inner(), pair_parts.inner + off);
        }
    }

    #[test]
    fn test_from_field() {
        let mut rng = rng();

        for _ in 0..1000 {
            let pair_parts = rng.random::<AtomicPairParts>();
            let field = Field::from_raw_parts(pair_parts.index_max, pair_parts.inner);
            let pair = AtomicPair::from(field);
            assert_eq!(pair.index_max, pair_parts.index_max);
            assert_eq!(pair.load_take().get_raw_inner(), pair_parts.inner);
            assert_eq!(pair.load_give().get_raw_inner(), pair_parts.inner);
        }
    }
}
