use super::{
    Error, Result,
    atomic::AtomicField,
    block::{AllocState, Block, ReserveState},
    entries::{ConsumingEntry, ProducingEntry},
    field::{Field, FieldConfig},
};
use std::{array, fmt::Debug, mem::MaybeUninit, sync::atomic::Ordering};

/*
! New
Fifo::<Val_t, NUM_BLOCKS: 2, BLOCK_SIZE: 5>::new():

? State
pField -> a
cField -> a
blocks: [a, b]

Block::new():
v Consumed (0)
v Reserved (0)
v Committed (0)
v Allocated (0)
[Uninit, Uninit, Uninit, Uninit, Uninit]

!Process of Fifo::push(val)

? Allocate an entry
block[a]:
v Consumed (0)
v Reserved (0)
v Committed (0)
|          v Allocated (1)
[Allocated, Uninit, Uninit, Uninit, Uninit]

? Write val into the allocated slot
block[a]:
v Consumed (0)
v Reserved (0)
|    v Committed (1)
|    v Allocated (1)
[val, Uninit, Uninit, Uninit, Uninit]

! Process of Fifo::pop()

? State
Fifo:
pField -> a
cField -> a
blocks: [a, b, c, d]

block[a]:
v Consumed (0)
v Reserved (0)
|    v Committed (1)
|    v Allocated (1)
[val, Uninit, Uninit, Uninit, Uninit]

? Reserve a slot
block[a]:
v Consumed (0)
|         v Reserved (1)
|         v Committed (1)
|         v Allocated (1)
[Reserved, Uninit, Uninit, Uninit, Uninit]

? Consume the slot
block[a]:
        v Consumed (1)
        v Reserved (1)
        v Committed (1)
        v Allocated (1)
[Uninit, Uninit, Uninit, Uninit, Uninit]

! Complex interaction 1: advancing cField with pop

? State
Fifo:
pField -> b
cField -> a
blocks: [a, b]

block[a]: (empty)
                                   v Consumed (5)
                                   v Reserved (5)
                                   v Committed (5)
                                   v Allocated (5)
[Uninit, Reserved, Val1, Val2, Val3]

block[b]: (half-empty)
v Consumed (0)
v Reserved (0)
|           v Committed (2)
|           |          v Allocated (3)
[Val4, Val5, Allocated, Uninit, Uninit]

? Attempt to reserve a slot
? See that block[cField (a)].reserved == BLOCK_SIZE (5)
? advance cField


*/

pub(crate) struct FastFifoInner<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> {
    phead: AtomicField<NUM_BLOCKS>,
    chead: AtomicField<NUM_BLOCKS>,
    blocks: [Block<T, BLOCK_SIZE>; NUM_BLOCKS],
}

#[rustfmt::skip]
unsafe impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Send for FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE> {}
#[rustfmt::skip]
unsafe impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Sync for FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE> {}

enum AdvancePheadState {
    Success,
    NoEntry,
    NotAvailable,
}

impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE> {
    pub fn new() -> Self {
        // There might be a better way to do this, since this is only a runtime check, but we can check NUM_BLOCKS at compile time.
        assert!(
            NUM_BLOCKS > 1,
            "If you want only one block, use a different Fifo."
        );

        Self {
            phead: Default::default(),
            chead: AtomicField::new(FieldConfig {
                ..Default::default()
            }),
            blocks: array::from_fn(|i| {
                if i == 0 {
                    Default::default()
                } else {
                    // {.idx = BLOCK_SIZE}
                    Block {
                        allocated: AtomicField::new(FieldConfig {
                            index: BLOCK_SIZE,
                            ..Default::default()
                        }),
                        committed: AtomicField::new(FieldConfig {
                            index: BLOCK_SIZE,
                            ..Default::default()
                        }),
                        reserved: AtomicField::new(FieldConfig {
                            // index: BLOCK_SIZE,
                            ..Default::default()
                        }),
                        consumed: AtomicField::new(FieldConfig {
                            index: BLOCK_SIZE,
                            ..Default::default()
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

    fn get_phead_and_block(&self) -> (Field<NUM_BLOCKS>, &Block<T, BLOCK_SIZE>) {
        let ph = self.phead.load(Ordering::Relaxed);
        (ph, &self.blocks[ph.get_index()])
    }

    fn advance_phead(&self, ph: Field<NUM_BLOCKS>) -> AdvancePheadState {
        let ref nblk = self.blocks[(ph.get_index() + 1) % NUM_BLOCKS];
        // /* retry-new begin
        let consumed = nblk.consumed.load(Ordering::Acquire);

        if consumed.get_version() < ph.get_version()
            || (consumed.get_version() == ph.get_version() && consumed.get_index() != BLOCK_SIZE)
        {
            let reserved = nblk.reserved.load(Ordering::Relaxed);

            if reserved.get_index() == consumed.get_index() {
                AdvancePheadState::NoEntry
            } else {
                AdvancePheadState::NotAvailable
            }
        }
        // */ // retry-new end
        /* drop-old begin
        let committed = nblk.committed.load(Ordering::Aquire);
        if committed.get_version() == ph.get_version() && committed.get_index() != BLOCK_SIZE {
            AdvancePheadState::NotAvailable
        }
        // */ // drop-old end
        else {
            let new_field = FieldConfig {
                version: ph.get_version() + 1,
                ..Default::default()
            }
            .into();

            nblk.committed.fetch_max(new_field, Ordering::Relaxed);
            nblk.allocated.fetch_max(new_field, Ordering::Relaxed);

            self.phead
                .fetch_max(ph.version_inc_add(1), Ordering::Relaxed);

            AdvancePheadState::Success
        }
    }

    fn get_chead_and_block(&self) -> (Field<NUM_BLOCKS>, &Block<T, BLOCK_SIZE>) {
        let ch = self.chead.load(Ordering::Relaxed);
        (ch, &self.blocks[ch.get_index()])
    }

    #[allow(unused_variables)]
    fn advance_chead(&self, ch: Field<NUM_BLOCKS>, version: usize) -> bool {
        let ref nblk = self.blocks[(ch.get_index() + 1) % NUM_BLOCKS];
        let committed = nblk.committed.load(Ordering::Acquire);

        // /* retry-new begin
        if committed.get_version() != ch.get_version() + 1 {
            return false;
        }
        let new_field = FieldConfig {
            version: ch.get_version() + 1,
            ..Default::default()
        }
        .into();
        nblk.consumed.fetch_max(new_field, Ordering::Relaxed);
        nblk.reserved.fetch_max(new_field, Ordering::Relaxed);
        // */ // retry-new end
        /* drop-old begin
        if committed.get_version() < version + if ch.get_index() == 0 { 1 } else { 0 } {
            return false;
        }
        nblk.reserved.fetch_max(
            FieldConfig {
                index: 0,
                version: committed.get_version(),
            },
            Ordering::Relaxed,
        );
        // */ // drop-old end

        self.chead
            .fetch_max(ch.version_inc_add(1), Ordering::Relaxed);
        true
    }

    /// Try to reserve a production entry
    pub fn get_producer_entry(&self) -> Result<ProducingEntry<'_, T, BLOCK_SIZE>> {
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
        self.get_producer_entry()
            .map(|mut entry| entry.produce_t_in_place(producer))
    }

    pub fn push(&self, val: T) -> Result<()> {
        self.push_in_place(|ptr| unsafe { ptr.write(val) })
    }

    pub fn get_consumer_entry(&self) -> Result<ConsumingEntry<'_, T, BLOCK_SIZE>> {
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
        self.get_consumer_entry()
            .map(|mut entry| entry.consume_t_in_place(consumer))
    }

    pub fn pop(&self) -> Result<T> {
        let mut uninit_mem = MaybeUninit::uninit();

        self.pop_in_place(|ptr| {
            uninit_mem.write(unsafe { ptr.read() });
        })
        .map(|()| unsafe { uninit_mem.assume_init() })
    }
}

impl<T: Debug, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Debug
    for FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(&self.blocks).finish()
    }
}

impl<T, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Drop
    for FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE>
{
    fn drop(&mut self) {
        self.blocks.iter_mut().for_each(Block::drop);
    }
}
