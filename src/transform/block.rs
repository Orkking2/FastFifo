use crate::transform::{atom_pair::AtomicPair, entries::EntryDescriptor, layer::Layer};
use std::{array, cell::UnsafeCell, mem::MaybeUninit};

#[repr(C)]
pub struct Block<T, const BLOCK_SIZE: usize> {
    atomics: [AtomicPair<BLOCK_SIZE>; 3],
    entries: [UnsafeCell<MaybeUninit<T>>; BLOCK_SIZE],
}

pub enum ReserveState<'a, T, const BLOCK_SIZE: usize> {
    Success(EntryDescriptor<'a, T, BLOCK_SIZE>),
    NotAvailable,
    BlockDone,
    Busy,
}

impl<T, const BLOCK_SIZE: usize> Default for Block<T, BLOCK_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const BLOCK_SIZE: usize> Block<T, BLOCK_SIZE> {
    pub fn new() -> Self {
        Self {
            atomics: array::from_fn(|_| AtomicPair::new()),
            entries: array::from_fn(|_| UnsafeCell::new(MaybeUninit::uninit())),
        }
    }

    pub fn get_atomics(&self, layer: Layer) -> &AtomicPair<BLOCK_SIZE> {
        &self.atomics[layer as usize]
    }

    pub fn get_current_chasing(
        &self,
        layer: Layer,
    ) -> (&AtomicPair<BLOCK_SIZE>, &AtomicPair<BLOCK_SIZE>) {
        (self.get_atomics(layer), self.get_atomics(layer.chasing()))
    }

    pub fn reserve_in_layer(&self, layer: Layer) -> ReserveState<'_, T, BLOCK_SIZE> {
        let (current, chasing) = self.get_current_chasing(layer);

        loop {
            let current_take = current.load_take();

            if current_take.get_index() >= BLOCK_SIZE {
                break ReserveState::BlockDone;
            } else {
                let chasing_give = chasing.load_give();

                if current_take.get_index() == chasing_give.get_index() {
                    break ReserveState::NotAvailable;
                } else {
                    let chasing_take = chasing.load_take();

                    if chasing_take.get_index() != chasing_give.get_index() {
                        break ReserveState::Busy;
                    } else {
                        if current.fetch_max_take(current_take.overflowing_add(1)) == current_take {
                            break ReserveState::Success(EntryDescriptor {
                                block: &self,
                                index: current_take.get_index(),
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
    pub unsafe fn get_ptr(&self, index: usize) -> *mut T {
        unsafe { self.entries[index].as_mut_unchecked() }.as_mut_ptr()
    }
}
