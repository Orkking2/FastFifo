use crate::{EntryDescription, atomic::Atomic, cursor::Cursor};
use std::{array, cell::UnsafeCell, fmt::Debug, mem::MaybeUninit, sync::atomic::Ordering};

pub struct Block<T, const BLOCK_SIZE: usize> {
    pub(crate) allocated: Atomic<Cursor<BLOCK_SIZE>>,
    pub(crate) committed: Atomic<Cursor<BLOCK_SIZE>>,
    pub(crate) reserved: Atomic<Cursor<BLOCK_SIZE>>,
    pub(crate) consumed: Atomic<Cursor<BLOCK_SIZE>>,
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
        if self.allocated.load(Ordering::SeqCst).get_offset() >= BLOCK_SIZE {
            AllocState::BlockDone
        } else {
            let old = self.allocated.fetch_add(1, Ordering::SeqCst).get_offset();

            if old >= BLOCK_SIZE {
                AllocState::BlockDone
            } else {
                AllocState::Allocated(EntryDescription {
                    block: &self,
                    offset: old,
                    version: 0,
                })
            }
        }
    }

    pub fn reserve_entry(&self) -> ReserveState<'_, T, BLOCK_SIZE> {
        loop {
            let reserved = self.reserved.load(Ordering::SeqCst);

            if reserved.get_offset() < BLOCK_SIZE {
                let committed = self.committed.load(Ordering::SeqCst);

                if reserved.get_offset() == committed.get_offset() {
                    break ReserveState::NoEntry;
                }
                if committed.get_offset() != BLOCK_SIZE {
                    let allocated = self.allocated.load(Ordering::SeqCst);
                    if allocated.get_offset() != committed.get_offset() {
                        break ReserveState::NotAvailable;
                    }
                }
                if self.reserved.fetch_max(reserved + 1, Ordering::SeqCst) == reserved {
                    break ReserveState::Reserved(EntryDescription {
                        block: &self,
                        offset: reserved.get_offset(),
                        version: reserved.get_version(),
                    });
                }
            } else {
                break ReserveState::BlockDone(reserved.get_version());
            }
        }
    }
}

impl<T: Debug, const BLOCK_SIZE: usize> Debug for Block<T, BLOCK_SIZE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // f.debug_struct("Block")
        //     .field("allocated", &self.allocated)
        //     .field("committed", &self.committed)
        //     .field("reserved", &self.reserved)
        //     .field("consumed", &self.consumed)
        //     .finish()
        
        //         v Consumed
        //         |         v Reserved
        //         |         |  v Committed
        //         |         |  |          v Allocated
        // [Uninit, Reserved, 0, Allocated, Uninit]

        let allocated = self.allocated.load(Ordering::Relaxed).get_offset();
        let committed = self.committed.load(Ordering::Relaxed).get_offset();
        let consumed = self.consumed.load(Ordering::Relaxed).get_offset();
        let reserved = self.reserved.load(Ordering::Relaxed).get_offset();

        f.debug_list()
            .entries(self.entries.iter().enumerate().map(|(i, t)| {
                if i < committed && i >= reserved {
                    format!("{:?}", unsafe { t.get().read().assume_init() })
                } else if i >= committed && i < allocated {
                    "Allocated".to_string()
                } else if i < reserved && i >= consumed {
                    "Reserved".to_string()
                } else {
                    "Uninit".to_string()
                }

                // if i < consumed.get_offset()
                //     || i >= allocated.get_offset()
                // {
                //     "Uninit".to_string()
                // } else if i >= committed.get_offset() {
                //     "Allocated".to_string()
                // } else if i >= reserved.get_offset() {
                //     format!("{:?}", unsafe { t.get().read().assume_init() })
                // } else {
                //     "Reserved".to_string()
                // }
            }))
            .finish()
    }
}
