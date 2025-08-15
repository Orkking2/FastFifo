#[cfg(feature = "debug")]
use tracing::{info, instrument, warn};

use crate::{
    atom_pair::AtomicPair,
    config::{FifoTag, IndexedDrop},
    entry_descriptor::EntryDescriptor,
    field::Field,
};
use std::{/*alloc::Allocator,*/ marker::PhantomData};

#[cfg(loom)]
use loom::cell::{MutPtr, UnsafeCell};

#[cfg(not(loom))]
use std::cell::UnsafeCell;

#[repr(C)]
pub struct Block<Tag: FifoTag, Inner: IndexedDrop<Tag> /*A: Allocator*/> {
    _phantom: PhantomData<(Tag /*A*/,)>,
    atomics: Box<[AtomicPair]>,
    entries: Box<[UnsafeCell<Inner>]>,
    block_size: usize,
}

pub enum ReserveState<'a, Tag: FifoTag, Inner: IndexedDrop<Tag> /*A: Allocator*/> {
    Success(EntryDescriptor<'a, Tag, Inner /*A*/>),
    NotAvailable,
    BlockDone,
    Busy,
}

impl<Tag: FifoTag, Inner: IndexedDrop<Tag> /*A: Allocator*/> Block<Tag, Inner /*A*/> {
    #[cfg_attr(feature = "debug", instrument(skip(block_size, /*alloc*/)))]
    pub fn new_in(block_size: usize /*alloc: &A*/) -> Self
    where
        Inner: Default,
    {
        Self {
            _phantom: PhantomData,
            atomics: {
                let mut vec = Vec::new(); // (alloc);
                vec.reserve(Tag::num_transformations());

                vec.extend((0..Tag::num_transformations()).map(|i| {
                    let field = Field::from_parts(block_size, 0, 0);

                    #[cfg(feature = "debug")]
                    info!("Atomics[{i}] = {field:?}");
                    let _ = i;

                    AtomicPair::from(field)
                }));

                vec.into_boxed_slice()
            },
            entries: {
                let mut vec = Vec::new(); // (alloc);
                vec.resize_with(block_size, || UnsafeCell::new(Inner::default()));

                vec.into_boxed_slice()
            },
            block_size,
        }
    }

    pub fn get_atomics(&self, tag: Tag) -> &AtomicPair {
        &self.atomics.as_ref()[tag.into()]
    }

    pub fn get_current_chasing(&self, tag: Tag) -> (&AtomicPair, &AtomicPair) {
        (self.get_atomics(tag), self.get_atomics(tag.chases()))
    }

    #[cfg_attr(feature = "debug", instrument(skip(self, tag)))]
    pub fn reserve_in_layer(&self, tag: Tag) -> ReserveState<'_, Tag, Inner /*A*/> {
        let (current, chasing) = self.get_current_chasing(tag);
        let producer_offset = if tag == Tag::producer() { 1 } else { 0 };

        loop {
            let current_take = current.load_take();

            #[cfg(feature = "debug")]
            info!(?current_take);

            if current_take.get_index() >= self.block_size {
                #[cfg(feature = "debug")]
                info!("BlockDone");
                break ReserveState::BlockDone;
            } else {
                let chasing_give = chasing.load_give();

                #[cfg(feature = "debug")]
                info!(?chasing_give);

                if current_take.get_version() >= chasing_give.get_version() + producer_offset {
                    if current_take.get_index() == chasing_give.get_index()
                        || current_take.get_version() > chasing_give.get_version() + producer_offset
                    {
                        #[cfg(feature = "debug")]
                        warn!("NotAvailable");
                        break ReserveState::NotAvailable;
                    } else {
                        let chasing_take = chasing.load_take();

                        #[cfg(feature = "debug")]
                        info!(?chasing_take);

                        if chasing_take.get_index() > chasing_give.get_index() {
                            #[cfg(feature = "debug")]
                            warn!("Busy");
                            break ReserveState::Busy;
                        }
                    }
                }

                let current_take_overflowing_add = current_take.overflowing_add(1);
                #[cfg(feature = "debug")]
                info!(?current_take_overflowing_add);

                let fetch_max_result = current.fetch_max_take(current_take_overflowing_add);
                #[cfg(feature = "debug")]
                info!(?fetch_max_result);

                if fetch_max_result == current_take {
                    break ReserveState::Success(EntryDescriptor {
                        block: &self,
                        index: current_take.get_index(),
                        tag,
                    });
                }
            }
        }
    }

    #[cfg(not(loom))]
    pub fn get_ptr(&self, index: usize) -> *mut Inner {
        self.entries.as_ref()[index].get()
    }

    #[cfg(loom)]
    pub fn get_ptr(&self, index: usize) -> MutPtr<Inner> {
        self.entries.as_ref()[index].get_mut()
    }

    pub fn drop_in(&mut self /*, alloc: &A*/) {
        let x = (0..Tag::num_transformations())
            .map(|i| {
                let atomic_pair = &self.atomics.as_ref()[i];
                let (give, take) = (atomic_pair.load_give(), atomic_pair.load_take());

                if give.get_index() < take.get_index() {
                    panic!("attempted to drop block while there exist incomplete transformations")
                } else {
                    give.get_index()
                }
            })
            .collect::<Vec<_>>();

        //         v [2].give (1)
        //         |         v [2].take (2)
        //         |         |           v [1].give (3)
        //         |         |           |            v [1].take (4)
        //         |         |           |            |          v [0].give (5)
        //         |         |           |            |          |          v [0].take (6)
        // [Uninit, Reserved, Post_Trans, Trans_Alloc, Pre_Trans, Allocated, Uninit] ->

        // Drop every set of entries between every current-chasing pair
        for i in 0..x.len() {
            let j = (i + x.len() - 1) % x.len();

            let current = x[i];
            let chasing = x[j];

            if current < chasing {
                for k in current..chasing {
                    #[cfg(not(loom))]
                    unsafe {
                        self.entries.as_mut()[k].get_mut().indexed_drop(i)
                    }
                    #[cfg(loom)]
                    unsafe {
                        self.entries.as_mut()[k].get_mut().deref().indexed_drop(i)
                    }
                }
            }
        }

        // Drop the set of entries outside of [x.first.index..x.last.index] with an index set intentionally to x.len()
        // which is usually out of range, inducing a forget rather than a drop for what is usually uninitialized data.
        //
        // Simply implementing a valid TryFrom<usize> for your UnionTag will change this behaviour to whatever you want!
        for k in (0..x[0]).chain(x[x.len() - 1]..self.entries.len()) {
            #[cfg(not(loom))]
            unsafe {
                self.entries.as_mut()[k].get_mut().indexed_drop(k)
            }
            #[cfg(loom)]
            unsafe {
                self.entries.as_mut()[k].get_mut().deref().indexed_drop(k)
            }
        }
    }
}

impl<Tag: FifoTag, Inner: IndexedDrop<Tag>> Drop for Block<Tag, Inner> {
    fn drop(&mut self) {
        self.drop_in()
    }
}
