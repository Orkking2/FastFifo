//         v Consumed (1)
//         |         v Reserved (2)
//         |         |           v Trans_committed (3)
//         |         |           |            v Trans_allocated (4)
//         |         |           |            |          v Committed (5)
//         |         |           |            |          |          v Allocated (6)
// [Uninit, Reserved, Post_Trans, Trans_Alloc, Pre_Trans, Allocated, Uninit] ->

use std::{
    mem::{ManuallyDrop, MaybeUninit},
    sync::Arc,
};

use crate::transform::{
    entries::{ConsumingEntry, ProducingEntry, TransformingEntry},
    error::Error,
    fifo::FastFifoInner,
};

pub mod entries;
pub mod error;

pub type Result<T> = std::result::Result<T, Error>;

mod atom_pair;
mod block;
mod fifo;
mod head;
mod layer;
mod wide_field;

/// For transforming, T must be this type
/// with generics A (input) and B (output)
#[repr(C)]
pub union InOutUnion<Input, Output> {
    input: ManuallyDrop<Input>,
    output: ManuallyDrop<Output>,
}

pub trait IsInOutUnion {
    const VALUE: bool;
}

default impl<T> IsInOutUnion for T {
    const VALUE: bool = false;
}

impl<Input, Output> IsInOutUnion for InOutUnion<Input, Output> {
    const VALUE: bool = true;
}

pub struct FastFifo<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>(
    Arc<FastFifoInner<T, NUM_BLOCKS, BLOCK_SIZE>>,
);

impl<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>
    FastFifo<T, NUM_BLOCKS, BLOCK_SIZE>
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>
    FastFifo<T, NUM_BLOCKS, BLOCK_SIZE>
{
    pub fn new() -> Self {
        todo!()
    }

    pub fn split(
        self,
    ) -> (
        SingleProducer<T, NUM_BLOCKS, BLOCK_SIZE>,
        MultiTransformer<T, NUM_BLOCKS, BLOCK_SIZE>,
        SingleConsumer<T, NUM_BLOCKS, BLOCK_SIZE>,
    ) {
        (
            SingleProducer(self.clone()),
            MultiTransformer(self.clone()),
            SingleConsumer(self.clone()),
        )
    }

    fn get_producer_entry(&self) -> Result<ProducingEntry<'_, T, BLOCK_SIZE>> {
        self.0.get_producer_entry()
    }

    fn get_consumer_entry(&self) -> Result<ConsumingEntry<'_, T, BLOCK_SIZE>> {
        self.0.get_consumer_entry()
    }

    fn get_transformer_entry(&self) -> Result<TransformingEntry<'_, T, BLOCK_SIZE>> {
        self.0.get_transformer_entry()
    }
}

/*
pub fn get_producer_entry(&self) -> Result<ProducingEntry<'_, T, BLOCK_SIZE>> {
    self.get_entry(Layer::Producer)
        .map(|entry_descriptor| ProducingEntry(entry_descriptor))
}

pub fn get_consumer_entry(&self) -> Result<ConsumingEntry<'_, T, BLOCK_SIZE>> {
    self.get_entry(Layer::Consumer)
        .map(|entry_descriptor| ConsumingEntry(entry_descriptor))
}

pub fn get_transformer_entry(&self) -> Result<TransformingEntry<'_, T, BLOCK_SIZE>> {
    self.get_entry(Layer::Transformer)
        .map(|entry_descriptor| TransformingEntry(entry_descriptor))
}
*/

pub struct SingleProducer<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>(
    FastFifo<T, NUM_BLOCKS, BLOCK_SIZE>,
);

impl<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>
    SingleProducer<T, NUM_BLOCKS, BLOCK_SIZE>
{
    pub fn get_producer_entry(&mut self) -> Result<ProducingEntry<'_, T, BLOCK_SIZE>> {
        self.0.get_producer_entry()
    }

    pub fn push_t_in_place<F: FnOnce(*mut T)>(&mut self, producer: F) -> Result<()> {
        self.get_producer_entry()
            .map(|mut producing_entry| producing_entry.push_t_in_place(producer))
    }

    pub fn push_t(&mut self, val: T) -> Result<()> {
        self.get_producer_entry().map(|mut producing_entry| producing_entry.push_t(val))
    }
}

impl<Input, Output, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>
    SingleProducer<InOutUnion<Input, Output>, NUM_BLOCKS, BLOCK_SIZE>
{
    pub fn push_in_place<F: FnOnce(*mut Input)>(&mut self, producer: F) -> Result<()> {
        self.get_producer_entry()
            .map(|mut producing_entry| producing_entry.produce_input_in_place(producer))
    }

    pub fn push(&mut self, val: Input) -> Result<()> {
        self.push_in_place(|ptr| unsafe { ptr.write(val) })
    }
}

pub struct SingleConsumer<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>(
    FastFifo<T, NUM_BLOCKS, BLOCK_SIZE>,
);

impl<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>
    SingleConsumer<T, NUM_BLOCKS, BLOCK_SIZE>
{
    pub fn get_consumer_entry(&mut self) -> Result<ConsumingEntry<'_, T, BLOCK_SIZE>> {
        self.0.get_consumer_entry()
    }

    pub fn pop_t_in_place<F: FnOnce(*mut T)>(&mut self, consumer: F) -> Result<()> {
        self.get_consumer_entry()
            .map(|mut producing_entry| producing_entry.consume_t_in_place(consumer))
    }

    pub fn pop_t(&mut self) -> Result<T> {
        let mut out = MaybeUninit::uninit();

        self.pop_t_in_place(|ptr| unsafe { out.write(ptr.read()); })
            .map(|()| unsafe { out.assume_init() })
    }
}

impl<Input, Output, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>
    SingleConsumer<InOutUnion<Input, Output>, NUM_BLOCKS, BLOCK_SIZE>
{
    pub fn pop_in_place<F: FnOnce(*mut Output)>(&mut self, producer: F) -> Result<()> {
        self.get_consumer_entry()
            .map(|mut producing_entry| producing_entry.consume_output_in_place(producer))
    }

    pub fn pop(&mut self, val: Input) -> Result<()> {
        self.pop_in_place(|ptr| unsafe { ptr.write(val) })
    }
}

pub struct MultiTransformer<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize>(
    FastFifo<T, NUM_BLOCKS, BLOCK_SIZE>,
);

impl<T: IsInOutUnion, const NUM_BLOCKS: usize, const BLOCK_SIZE: usize> Clone
    for MultiTransformer<T, NUM_BLOCKS, BLOCK_SIZE>
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
