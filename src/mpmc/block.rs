use crate::field::FieldConfig;

use super::{atomic::AtomicField, entries::EntryDescription};
use std::{fmt::Debug, mem::MaybeUninit, sync::atomic::Ordering};

pub struct Block<T> {
    pub(crate) allocated: AtomicField,
    pub(crate) committed: AtomicField,
    pub(crate) reserved: AtomicField,
    pub(crate) consumed: AtomicField,
    pub(crate) block_size: usize,
    pub(crate) entries: *mut [MaybeUninit<T>],
}

pub enum AllocState<'a, T> {
    Allocated(EntryDescription<'a, T>),
    BlockDone,
}

pub enum ReserveState<'a, T> {
    Reserved(EntryDescription<'a, T>),
    NoEntry,
    NotAvailable,
    BlockDone(usize),
}

impl<T> Block<T> {
    pub fn new(block_size: usize) -> Self {
        Self {
            allocated: AtomicField::new(FieldConfig {
                index_max: block_size,
                version: 0,
                index: 0,
            }),
            committed: AtomicField::new(FieldConfig {
                index_max: block_size,
                version: 0,
                index: 0,
            }),
            reserved: AtomicField::new(FieldConfig {
                index_max: block_size,
                version: 0,
                index: 0,
            }),
            consumed: AtomicField::new(FieldConfig {
                index_max: block_size,
                version: 0,
                index: 0,
            }),
            entries: Box::into_raw({
                let mut vec = Vec::with_capacity(block_size);
                vec.extend((0..block_size).map(|_| MaybeUninit::uninit()));
                vec.into_boxed_slice()
            }),
            block_size,
        }
    }

    pub fn new_full(block_size: usize) -> Self {
        Self {
            allocated: AtomicField::new(FieldConfig {
                index_max: block_size,
                version: 0,
                index: block_size,
            }),
            committed: AtomicField::new(FieldConfig {
                index_max: block_size,
                version: 0,
                index: block_size,
            }),
            reserved: AtomicField::new(FieldConfig {
                index_max: block_size,
                version: 0,
                index: block_size,
            }),
            consumed: AtomicField::new(FieldConfig {
                index_max: block_size,
                version: 0,
                index: block_size,
            }),
            entries: Box::into_raw({
                let mut vec = Vec::with_capacity(block_size);
                vec.extend((0..block_size).map(|_| MaybeUninit::uninit()));
                vec.into_boxed_slice()
            }),
            block_size,
        }
    }

    pub fn allocate_entry(&self) -> AllocState<'_, T> {
        if self.allocated.load(Ordering::Relaxed).get_index() >= self.block_size {
            AllocState::BlockDone
        } else {
            let old = self.allocated.fetch_add(1, Ordering::Relaxed).get_index();

            if old >= self.block_size {
                AllocState::BlockDone
            } else {
                AllocState::Allocated(EntryDescription {
                    block: &self,
                    index: old,
                    version: 0,
                })
            }
        }
    }

    pub fn reserve_entry(&self) -> ReserveState<'_, T> {
        loop {
            let reserved = self.reserved.load(Ordering::Relaxed);

            if reserved.get_index() < self.block_size {
                // All previous writes in this block must be visible before this load.
                let committed = self.committed.load(Ordering::Acquire);

                if reserved.get_index() == committed.get_index() {
                    break ReserveState::NoEntry;
                }
                if committed.get_index() != self.block_size {
                    let allocated = self.allocated.load(Ordering::Relaxed);
                    if allocated.get_index() != committed.get_index() {
                        break ReserveState::NotAvailable;
                    }
                }
                if self
                    .reserved
                    .fetch_max(reserved.overflowing_add(1), Ordering::Relaxed)
                    == reserved
                {
                    break ReserveState::Reserved(EntryDescription {
                        block: &self,
                        index: reserved.get_index(),
                        version: reserved.get_version(),
                    });
                }
            } else {
                break ReserveState::BlockDone(reserved.get_version());
            }
        }
    }

    /// Drop the valid values inside self.
    pub(crate) fn drop(&mut self) {
        let Block {
            allocated,
            committed,
            reserved,
            consumed,
            entries,
            block_size,
        } = self;

        let _ = block_size;

        let allocated = allocated.load(Ordering::Relaxed).get_index();
        let committed = committed.load(Ordering::Relaxed).get_index();
        let reserved = reserved.load(Ordering::Relaxed).get_index();
        let consumed = consumed.load(Ordering::Relaxed).get_index();

        unsafe { &mut **entries }
            .iter_mut()
            .enumerate()
            .for_each(|(i, t)| {
                if i < committed && i >= reserved {
                    // This T is valid, so we must manually drop it
                    std::mem::drop(unsafe {
                        (t as *const MaybeUninit<T>).read().assume_init_read()
                    })
                } else if (i >= committed && i < allocated) || (i < reserved && i >= consumed) {
                    // This value is either allocated or reserved (in use)
                    // This is undefined behaviour, so we panic
                    panic!(
                        "Dropping block while it has an {} value",
                        if i >= committed && i < allocated {
                            "allocated"
                        } else {
                            "reserved"
                        }
                    )
                } else {
                    /* ignore -- uninit */
                }
            })
    }
}

impl<T: Debug> Debug for Block<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // f.debug_struct("Block")
        //     .field("allocated", &self.allocated)
        //     .field("committed", &self.committed)
        //     .field("reserved", &self.reserved)
        //     .field("consumed", &self.consumed)
        //     .finish()?;

        //         v Consumed (1)
        //         |         v Reserved (2)
        //         |         |  v Committed (3)
        //         |         |  |          v Allocated (4)
        // [Uninit, Reserved, 0, Allocated, Uninit]

        let allocated = self.allocated.load(Ordering::Relaxed).get_index();
        let committed = self.committed.load(Ordering::Relaxed).get_index();
        let consumed = self.consumed.load(Ordering::Relaxed).get_index();
        let reserved = self.reserved.load(Ordering::Relaxed).get_index();

        f.debug_list()
            .entries(unsafe { &*self.entries }.iter().enumerate().map(|(i, t)| {
                if i >= allocated
                    || (allocated >= self.block_size
                        && committed >= self.block_size
                        && consumed >= self.block_size
                        && reserved >= self.block_size)
                {
                    format!("Uninit")
                } else if i >= committed {
                    format!("Allocated")
                } else if i >= reserved
                    || (consumed == self.block_size && reserved == self.block_size)
                {
                    format!("{:?}", unsafe {
                        (t as *const MaybeUninit<T>).read().assume_init_read()
                    })
                } else if i >= consumed {
                    format!("Reserved")
                } else {
                    format!("Uninit")
                }
            }))
            .finish()
    }
}
