use crate::transform::{InOutUnion, block::Block, layer::Layer};
use std::mem::{ManuallyDrop, MaybeUninit};

pub(crate) struct EntryDescriptor<'a, T, const BLOCK_SIZE: usize> {
    pub(crate) block: &'a Block<T, BLOCK_SIZE>,
    pub(crate) index: usize,
}

impl<'a, T, const BLOCK_SIZE: usize> EntryDescriptor<'a, T, BLOCK_SIZE> {
    fn modify_t_in_place<F: FnOnce(*mut T)>(&mut self, modifier: F) {
        modifier(unsafe { self.block.get_ptr(self.index) })
    }
}


pub struct ProducingEntry<'a, T, const BLOCK_SIZE: usize>(
    pub(crate) EntryDescriptor<'a, T, BLOCK_SIZE>,
);

impl<'a, T, const BLOCK_SIZE: usize> ProducingEntry<'a, T, BLOCK_SIZE> {
    pub fn push_t_in_place<F: FnOnce(*mut T)>(&mut self, producer: F) {
        self.0.modify_t_in_place(producer);
    }

    pub fn push_t(&mut self, val: T) {
        self.push_t_in_place(|ptr| unsafe { ptr.write(val) });
    }
}

impl<'a, Input, Output, const BLOCK_SIZE: usize>
    ProducingEntry<'a, InOutUnion<Input, Output>, BLOCK_SIZE>
{
    pub fn produce_input_in_place<F: FnOnce(*mut Input)>(&mut self, producer: F) {
        self.push_t_in_place(|ptr| unsafe {
            producer(&mut (*ptr).input as *mut ManuallyDrop<Input> as *mut Input);
        });
    }
}

impl<'a, T, const BLOCK_SIZE: usize> Drop for ProducingEntry<'a, T, BLOCK_SIZE> {
    fn drop(&mut self) {
        self.0.block.get_atomics(Layer::Producer).incr_give();
    }
}

pub struct TransformingEntry<'a, T, const BLOCK_SIZE: usize>(
    pub(crate) EntryDescriptor<'a, T, BLOCK_SIZE>,
);

impl<'a, T, const BLOCK_SIZE: usize> TransformingEntry<'a, T, BLOCK_SIZE> {
    pub fn transform_t_in_place<F: FnOnce(*mut T)>(&mut self, transformer: F) {
        self.0.modify_t_in_place(transformer);
    }

    pub fn transform_t<F: FnOnce(T) -> T>(&mut self, transformer: F) {
        self.transform_t_in_place(|ptr| unsafe { ptr.write(transformer(ptr.read())) });
    }
}

impl<'a, Input, Output, const BLOCK_SIZE: usize>
    TransformingEntry<'a, InOutUnion<Input, Output>, BLOCK_SIZE>
{
    pub fn transform<F: FnOnce(Input) -> Output>(&mut self, transformer: F) {
        self.transform_t_in_place(|ptr| unsafe {
            ptr.write(InOutUnion {
                output: ManuallyDrop::new(transformer(ManuallyDrop::<Input>::into_inner(
                    ptr.read().input,
                ))),
            })
        });
    }
}

impl<'a, T, const BLOCK_SIZE: usize> Drop for TransformingEntry<'a, T, BLOCK_SIZE> {
    fn drop(&mut self) {
        self.0.block.get_atomics(Layer::Transformer).incr_give();
    }
}

pub struct ConsumingEntry<'a, T, const BLOCK_SIZE: usize>(
    pub(crate) EntryDescriptor<'a, T, BLOCK_SIZE>,
);

impl<'a, T, const BLOCK_SIZE: usize> ConsumingEntry<'a, T, BLOCK_SIZE> {
    pub fn pop_t_in_place<F: FnOnce(*mut T)>(&mut self, consumer: F) {
        self.0.modify_t_in_place(consumer);
    }

    pub fn consume_t(&mut self) -> T {
        let mut out = MaybeUninit::uninit();

        self.pop_t_in_place(|ptr| unsafe {
            out.write(ptr.read());
        });

        unsafe { out.assume_init() }
    }
}

impl<'a, Input, Output, const BLOCK_SIZE: usize>
    ConsumingEntry<'a, InOutUnion<Input, Output>, BLOCK_SIZE>
{
    pub fn pop_in_place<F: FnOnce(*mut Output)>(&mut self, consumer: F) {
        self.pop_t_in_place(|ptr| unsafe {
            consumer(&mut (*ptr).output as *mut ManuallyDrop<Output> as *mut Output)
        });
    }

    pub fn pop(&mut self) -> Output {
        let mut out = MaybeUninit::uninit();

        self.pop_in_place(|ptr| unsafe {
            out.write(ptr.read());
        });

        unsafe { out.assume_init() }
    }
}

impl<'a, T, const BLOCK_SIZE: usize> Drop for ConsumingEntry<'a, T, BLOCK_SIZE> {
    fn drop(&mut self) {
        self.0.block.get_atomics(Layer::Consumer).incr_give();
    }
}
