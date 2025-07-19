#![feature(unsafe_cell_access)]
// ! temp
#![allow(dead_code)]
#![allow(unused_variables)]
#![feature(thread_sleep_until)]

use crate::{
    atomic::Atomic,
    block::{AllocState, Block, ReserveState},
    cursor::CursorConfig,
    error::Error,
    head::{Head, HeadConfig},
};
use std::{array, fmt::Debug, mem::MaybeUninit, sync::atomic::Ordering};

mod atomic;
mod block;
mod cursor;
mod error;
mod head;
mod test;
mod util;

pub type Result<T> = std::result::Result<T, Error>;

pub struct FastFifo<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> {
    phead: Atomic<Head<NUM_BLOCKS>>,
    chead: Atomic<Head<NUM_BLOCKS>>,
    blocks: [Block<T, BLOCK_SIZE>; NUM_BLOCKS],
}

unsafe impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Send for FastFifo<T, NUM_BLOCKS, BLOCK_SIZE> {}
unsafe impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Sync for FastFifo<T, NUM_BLOCKS, BLOCK_SIZE> {}

#[repr(transparent)]
pub struct ProducingEntry<'a, T, const BLOCK_SIZE: usize>(EntryDescription<'a, T, BLOCK_SIZE>);

impl<'a, T, const BLOCK_SIZE: usize> ProducingEntry<'a, T, BLOCK_SIZE> {
    pub fn produce_t_in_place<F: FnOnce(*mut T)>(&mut self, producer: F) {
        self.0.modify_t_in_place(producer);
    }
}

impl<'a, T, const BLOCK_SIZE: usize> Drop for ProducingEntry<'a, T, BLOCK_SIZE> {
    fn drop(&mut self) {
        self.0.block.committed.fetch_add(1, Ordering::SeqCst);
    }
}

#[repr(transparent)]
pub struct ConsumingEntry<'a, T, const BLOCK_SIZE: usize>(EntryDescription<'a, T, BLOCK_SIZE>);

impl<'a, T, const BLOCK_SIZE: usize> ConsumingEntry<'a, T, BLOCK_SIZE> {
    pub fn consume_t_in_place<F: FnOnce(*mut T)>(&mut self, consumer: F) {
        self.0.modify_t_in_place(consumer);
    }
}

impl<'a, T, const BLOCK_SIZE: usize> Drop for ConsumingEntry<'a, T, BLOCK_SIZE> {
    fn drop(&mut self) {
        self.0.block.consumed.fetch_add(1, Ordering::SeqCst);
    }
}

struct EntryDescription<'a, T, const BLOCK_SIZE: usize> {
    pub(crate) block: &'a Block<T, BLOCK_SIZE>,
    pub(crate) offset: usize,
    pub(crate) version: usize,
}

impl<'a, T, const BLOCK_SIZE: usize> EntryDescription<'a, T, BLOCK_SIZE> {
    /// Modify *mut T in-place
    pub fn modify_t_in_place<F: FnOnce(*mut T)>(&mut self, modifier: F) {
        modifier(unsafe { self.block.entries[self.offset].as_mut_unchecked() }.as_mut_ptr())
    }
}

enum AdvancePheadState {
    Success,
    NoEntry,
    NotAvailable,
}

impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Default
    for FastFifo<T, NUM_BLOCKS, BLOCK_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> FastFifo<T, NUM_BLOCKS, BLOCK_SIZE> {
    pub fn new() -> Self {
        // There might be a better way to do this, since this is only a runtime check, but we can check NUM_BLOCKS at compile time.
        assert!(
            NUM_BLOCKS > 1,
            "If you want only one block, use a different Fifo."
        );

        Self {
            phead: Atomic::new(HeadConfig {
                index: 0,
                version: 0,
            }),
            chead: Atomic::new(HeadConfig {
                index: NUM_BLOCKS - 1,
                version: 0,
            }),
            blocks: array::from_fn(|i| {
                if i == NUM_BLOCKS - 1 {
                    // This last block is where chead points,
                    // it is empty from the perspective of the consumer
                    Block {
                        reserved: Atomic::new(CursorConfig {
                            version: 0,
                            offset: BLOCK_SIZE,
                        }),
                        consumed: Atomic::new(CursorConfig {
                            version: 0,
                            offset: BLOCK_SIZE,
                        }),
                        ..Default::default()
                    }
                } else {
                    // All other blocks are empty from the perspective of the producer
                    Block {
                        consumed: Atomic::new(CursorConfig {
                            version: 0,
                            offset: BLOCK_SIZE,
                        }),
                        ..Default::default()
                    }
                }
            }),
        }
    }

    pub const fn capacity() -> usize {
        NUM_BLOCKS * BLOCK_SIZE
    }

    fn get_phead_and_block(&self) -> (Head<NUM_BLOCKS>, &Block<T, BLOCK_SIZE>) {
        let ph = self.phead.load(Ordering::SeqCst);
        (ph, &self.blocks[ph.get_index()])
    }

    fn advance_phead(&self, ph: Head<NUM_BLOCKS>) -> AdvancePheadState {
        let ref nblk = self.blocks[(ph.get_index() + 1) % NUM_BLOCKS];
        // /* retry-new begin
        let consumed = nblk.consumed.load(Ordering::SeqCst);

        if consumed.get_version() < ph.get_version()
            || (consumed.get_version() == ph.get_version() && consumed.get_offset() != BLOCK_SIZE)
        {
            let reserved = nblk.reserved.load(Ordering::SeqCst);

            if reserved.get_offset() == consumed.get_offset() {
                AdvancePheadState::NoEntry
            } else {
                AdvancePheadState::NotAvailable
            }
        }
        // */ // retry-new end
        /* drop-old begin
        let cmtd = nblk.committed.load(Ordering::SeqCst);
        if cmtd.get_version() == ph.get_version() && cmtd.get_offset() != BLOCK_SIZE {
            AdvancePheadState::NotAvailable
        }
        // */ // drop-old end
        else {
            let new_cursor = CursorConfig {
                offset: 0,
                version: ph.get_version() + 1,
            }
            .into();

            nblk.committed.fetch_max(new_cursor, Ordering::SeqCst);
            nblk.allocated.fetch_max(new_cursor, Ordering::SeqCst);

            self.phead.fetch_max(ph + 1, Ordering::SeqCst);

            AdvancePheadState::Success
        }
    }

    fn get_chead_and_block(&self) -> (Head<NUM_BLOCKS>, &Block<T, BLOCK_SIZE>) {
        let ch = self.chead.load(Ordering::SeqCst);
        (ch, &self.blocks[ch.get_index()])
    }

    fn advance_chead(&self, ch: Head<NUM_BLOCKS>, version: usize) -> bool {
        let ref nblk = self.blocks[(ch.get_index() + 1) % NUM_BLOCKS];
        let committed = nblk.committed.load(Ordering::SeqCst);

        // /* retry-new begin
        if committed.get_version() != ch.get_version() {
            return false;
        }
        let new_cursor = CursorConfig {
            offset: 0,
            version: ch.get_version() + 1,
        }
        .into();
        nblk.consumed.fetch_max(new_cursor, Ordering::SeqCst);
        nblk.reserved.fetch_max(new_cursor, Ordering::SeqCst);
        // */ // retry-new end
        /* drop-old begin
        if committed.get_version() < version + if ch.get_index() == 0 { 1 } else { 0 } {
            return false;
        }
        nblk.reserved.fetch_max(
            CursorConfig {
                offset: 0,
                version: committed.get_version(),
            },
            Ordering::SeqCst,
        );
        // */ // drop-old end

        self.chead.fetch_max(ch + 1, Ordering::SeqCst);
        true
    }

    /// Try to reserve a production entry
    pub fn try_get_producer_entry(&self) -> Result<ProducingEntry<'_, T, BLOCK_SIZE>> {
        loop {
            let (ph, blk) = self.get_phead_and_block();
            match blk.allocate_entry() {
                AllocState::Allocated(entry_description) => {
                    break Ok(ProducingEntry(entry_description));
                }
                AllocState::BlockDone => match self.advance_phead(ph) {
                    AdvancePheadState::NoEntry => break Err(Error::Full),
                    AdvancePheadState::NotAvailable => break Err(Error::Busy),
                    AdvancePheadState::Success => { /* continue loop */ }
                },
            }
        }
    }

    /// F produces T at address *mut T
    pub fn push_in_place<F: FnOnce(*mut T)>(&self, producer: F) -> Result<()> {
        self.try_get_producer_entry()
            .map(|mut entry| entry.produce_t_in_place(producer))
    }

    pub fn push(&self, val: T) -> Result<()> {
        self.push_in_place(|ptr| unsafe { ptr.write(val) })
    }

    pub fn try_get_consumer_entry(&self) -> Result<ConsumingEntry<'_, T, BLOCK_SIZE>> {
        loop {
            let (ch, blk) = self.get_chead_and_block();
            match blk.reserve_entry() {
                ReserveState::BlockDone(version) => {
                    if !self.advance_chead(ch, version) {
                        break Err(Error::Empty);
                    } else {
                        /* continue loop */
                    }
                }
                ReserveState::Reserved(entry_description) => {
                    break Ok(ConsumingEntry(entry_description));
                }
                ReserveState::NoEntry => break Err(Error::Empty),
                ReserveState::NotAvailable => break Err(Error::Busy),
            }
        }
    }

    /// F consumes T at address *mut T
    pub fn pop_in_place<F: FnOnce(*mut T)>(&self, consumer: F) -> Result<()> {
        self.try_get_consumer_entry()
            .map(|mut entry| entry.consume_t_in_place(consumer))
    }

    pub fn pop(&self) -> Result<T> {
        let mut uninit_mem = MaybeUninit::uninit();

        self.pop_in_place(|ptr| {
            uninit_mem.write(unsafe { ptr.read() });
        })
        .map(|_| unsafe { uninit_mem.assume_init() })
    }
}

impl<T: Debug, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Debug
    for FastFifo<T, NUM_BLOCKS, BLOCK_SIZE>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(&self.blocks).finish()
    }
}
