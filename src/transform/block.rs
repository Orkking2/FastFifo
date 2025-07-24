use crate::transform::{
    atom_pair::AtomicPair,
    config::FifoConfig,
    config::{FifoTag, IndexedDrop},
    entry_descriptor::EntryDescriptor,
};
use std::{array, cell::UnsafeCell};

#[repr(C)]
pub struct Block<Config: FifoConfig>
where
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    atomics: [AtomicPair<{ <Config as FifoConfig>::BLOCK_SIZE }>; {
        <Config as FifoConfig>::NUM_TRANSFORMATIONS
    }],
    entries: [UnsafeCell<<Config as FifoConfig>::Inner>; <Config as FifoConfig>::BLOCK_SIZE],
}

pub enum ReserveState<'a, Config: FifoConfig>
where
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    Success(EntryDescriptor<'a, Config>),
    NotAvailable,
    BlockDone,
    Busy,
}

impl<Config: FifoConfig> Default for Block<Config>
where
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Config: FifoConfig> Block<Config>
where
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    pub fn new() -> Self {
        Self {
            atomics: array::from_fn(|_| AtomicPair::new()),
            entries: array::from_fn(|_| UnsafeCell::new(Default::default())),
        }
    }

    pub fn full_consumer() -> Self {
        Self {
            atomics: array::from_fn(|i| {
                if i + 1 == <Config as FifoConfig>::NUM_TRANSFORMATIONS {
                    AtomicPair::full()
                } else {
                    AtomicPair::new()
                }
            }),
            entries: array::from_fn(|_| UnsafeCell::new(Default::default())),
        }
    }

    pub fn get_atomics(
        &self,
        tag: <Config as FifoConfig>::Tag,
    ) -> &AtomicPair<{ <Config as FifoConfig>::BLOCK_SIZE }> {
        &self.atomics[tag.into()]
    }

    pub fn get_current_chasing(
        &self,
        tag: <Config as FifoConfig>::Tag,
    ) -> (
        &AtomicPair<{ <Config as FifoConfig>::BLOCK_SIZE }>,
        &AtomicPair<{ <Config as FifoConfig>::BLOCK_SIZE }>,
    ) {
        (self.get_atomics(tag), self.get_atomics(tag.chases()))
    }

    pub fn reserve_in_tag(&self, tag: <Config as FifoConfig>::Tag) -> ReserveState<'_, Config> {
        let (current, chasing) = self.get_current_chasing(tag);

        loop {
            let current_take = current.load_take();

            if current_take.get_index() >= <Config as FifoConfig>::BLOCK_SIZE {
                break ReserveState::BlockDone;
            } else {
                let chasing_give = chasing.load_give();

                if current_take.get_index() == chasing_give.get_index() {
                    println!(
                        "current tag {} chasing {} current_take index {} == chasing_give index {}",
                        tag.into(),
                        tag.chases().into(),
                        current_take.get_index(),
                        chasing_give.get_index()
                    );

                    break ReserveState::NotAvailable;
                } else {
                    let chasing_take = chasing.load_take();

                    if chasing_take.get_index() > chasing_give.get_index() {
                        break ReserveState::Busy;
                    } else {
                        if current.fetch_max_take(current_take.overflowing_add(1)) == current_take {
                            break ReserveState::Success(EntryDescriptor {
                                block: &self,
                                index: current_take.get_index(),
                                tag,
                            });
                        } else {
                            continue;
                        }
                    }
                }
            }
        }
    }

    /// # Safety
    /// This must be the only concurrent access of self.entries[index]
    pub unsafe fn get_ptr(&self, index: usize) -> *mut <Config as FifoConfig>::Inner {
        self.entries[index].get()
    }
}

impl<Config: FifoConfig> Drop for Block<Config>
where
    [(); <Config as FifoConfig>::BLOCK_SIZE]:,
    [(); <Config as FifoConfig>::NUM_BLOCKS]:,
    [(); <Config as FifoConfig>::NUM_TRANSFORMATIONS]:,
{
    fn drop(&mut self) {
        let x: [usize; <Config as FifoConfig>::NUM_TRANSFORMATIONS] = array::from_fn(|i| {
            let ref atomic_pair = self.atomics[i];
            let (give, take) = (atomic_pair.load_give(), atomic_pair.load_take());

            

            if give.get_index() < take.get_index() {
                panic!("attempted to drop block while there exist incomplete transformations")
            } else {
                give.get_index()
            }
        });

        //         v [2].give (1)
        //         |         v [2].take (2)
        //         |         |           v [1].give (3)
        //         |         |           |            v [1].take (4)
        //         |         |           |            |          v [0].give (5)
        //         |         |           |            |          |          v [0].take (6)
        // [Uninit, Reserved, Post_Trans, Trans_Alloc, Pre_Trans, Allocated, Uninit] ->

        let indexed_drop_entry = |entry: &UnsafeCell<<Config as FifoConfig>::Inner>, index| unsafe {
            entry.get().read().indexed_drop(index)
        };

        // Drop every set of entries between every current-chasing pair
        for i in 0..x.len() {
            let j = (i + x.len() - 1) % x.len();

            let current = x[i];
            let chasing = x[j];

            if current < chasing {
                for k in current..chasing {
                    indexed_drop_entry(&self.entries[k], i);
                }
            }
        }

        // Drop the set of entries outside of [x.first.index..x.last.index] with an index set intentionally to x.len()
        // which is usually out of range, inducing a forget rather than a drop for what is usually uninitialized data
        //
        // Simply implementing a valid TryFrom<usize> for your UnionTag will change this behaviour to whatever you want!
        for k in (0..x[0]).chain(x[x.len() - 1]..self.entries.len()) {
            indexed_drop_entry(&self.entries[k], x.len());
        }
    }
}
