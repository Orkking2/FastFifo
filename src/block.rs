use crate::{atomic::AtomicField, entries::EntryDescription};
use std::{array, cell::UnsafeCell, fmt::Debug, mem::MaybeUninit, sync::atomic::Ordering};

pub struct Block<T, const BLOCK_SIZE: usize> {
    pub(crate) allocated: AtomicField<BLOCK_SIZE>,
    pub(crate) committed: AtomicField<BLOCK_SIZE>,
    pub(crate) reserved: AtomicField<BLOCK_SIZE>,
    pub(crate) consumed: AtomicField<BLOCK_SIZE>,
    pub(crate) entries: [UnsafeCell<MaybeUninit<T>>; BLOCK_SIZE],
}

pub enum AllocState<'a, T, const BLOCK_SIZE: usize> {
    Allocated(EntryDescription<'a, T, BLOCK_SIZE>),
    BlockDone,
}

pub enum ReserveState<'a, T, const BLOCK_SIZE: usize> {
    Reserved(EntryDescription<'a, T, BLOCK_SIZE>),
    NoEntry,
    NotAvailable,
    BlockDone(usize),
}

impl<T, const BLOCK_SIZE: usize> Default for Block<T, BLOCK_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const BLOCK_SIZE: usize> Block<T, BLOCK_SIZE> {
    pub fn new() -> Self {
        Self {
            allocated: Default::default(),
            committed: Default::default(),
            reserved: Default::default(),
            consumed: Default::default(),
            entries: array::from_fn(|_| UnsafeCell::new(MaybeUninit::uninit())),
        }
    }

    pub fn allocate_entry(&self) -> AllocState<'_, T, BLOCK_SIZE> {
        if self.allocated.load(Ordering::Relaxed).get_index() >= BLOCK_SIZE {
            AllocState::BlockDone
        } else {
            let old = self.allocated.fetch_add(1, Ordering::Relaxed).get_index();

            if old >= BLOCK_SIZE {
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

    pub fn reserve_entry(&self) -> ReserveState<'_, T, BLOCK_SIZE> {
        loop {
            let reserved = self.reserved.load(Ordering::Relaxed);

            if reserved.get_index() < BLOCK_SIZE {
                // All previous writes in this block must be visible before this load.
                let committed = self.committed.load(Ordering::Acquire);

                if reserved.get_index() == committed.get_index() {
                    break ReserveState::NoEntry;
                }
                if committed.get_index() != BLOCK_SIZE {
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
        } = self;

        let allocated = allocated.load(Ordering::Relaxed).get_index();
        let committed = committed.load(Ordering::Relaxed).get_index();
        let reserved = reserved.load(Ordering::Relaxed).get_index();
        let consumed = consumed.load(Ordering::Relaxed).get_index();

        entries.iter_mut().enumerate().for_each(|(i, t)| {
            if i < committed && i >= reserved {
                // This T is valid, so we must manually drop it
                std::mem::drop(unsafe { t.get().read().assume_init_read() })
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

impl<T: Debug, const BLOCK_SIZE: usize> Debug for Block<T, BLOCK_SIZE> {
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
            .entries(self.entries.iter().enumerate().map(|(i, t)| {
                if i >= allocated
                    || (allocated >= BLOCK_SIZE
                        && committed >= BLOCK_SIZE
                        && consumed >= BLOCK_SIZE
                        && reserved >= BLOCK_SIZE)
                {
                    format!("Uninit")
                } else if i >= committed {
                    format!("Allocated")
                } else if i >= reserved || (consumed == BLOCK_SIZE && reserved == BLOCK_SIZE) {
                    format!("{:?}", unsafe { t.get().read().assume_init() })
                } else if i >= consumed {
                    format!("Reserved")
                } else {
                    format!("Uninit")
                }
            }))
            .finish()
    }
}
