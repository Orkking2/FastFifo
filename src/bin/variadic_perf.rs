#![feature(thread_sleep_until)]
// #![feature(allocator_api)]

#[cfg(feature = "debug")]
use tracing::error;

use clap::Parser;
use fastfifo::generate_union;
use std::{
    fs::File,
    thread::sleep_until,
    time::{Duration, Instant},
};
use tracing::{Level, info, span};
use tracing_appender::non_blocking;
use tracing_subscriber::{
    EnvFilter, Registry, fmt::layer, layer::SubscriberExt, util::SubscriberInitExt,
};

#[derive(Parser, Debug)]
struct Cli {
    #[arg(short = 'b', long)]
    block_size: usize,

    #[arg(short = 'n', long)]
    num_blocks: usize,

    #[arg(short = 'o', long)]
    nops: usize,

    #[arg(short = 't', long)]
    num_trans_threads: Option<usize>,

    #[arg(short = 'l', long)]
    log_file: Option<String>,
}

// generate_union! {
//     pub InOutUnion<Input, Output> {
//         Producer: Input, atomic = false;
//         Transformer: Output, atomic = true;
//         Consumer: (), atomic = false;
//     }
// }

pub union InOutUnion<Input, Output> {
    producer: ::core::mem::ManuallyDrop<Input>,
    transformer: ::core::mem::ManuallyDrop<Output>,
    consumer: ::core::mem::ManuallyDrop<()>,
}
impl<Input, Output> ::std::default::Default for InOutUnion<Input, Output> {
    fn default() -> Self {
        Self {
            consumer: ::core::mem::ManuallyDrop::<()>::default(),
        }
    }
}
impl<Input, Output> ::core::convert::From<()> for InOutUnion<Input, Output> {
    fn from(val: ()) -> Self {
        Self {
            consumer: ::core::mem::ManuallyDrop::new(val),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum InOutUnionTag {
    Producer = 0usize,
    Transformer = 1usize,
    Consumer = 2usize,
}
impl ::core::convert::From<InOutUnionTag> for usize {
    fn from(val: InOutUnionTag) -> usize {
        val as usize
    }
}
#[derive(Debug)]
pub struct InOutUnionTagTryFromError(usize);
impl ::std::fmt::Display for InOutUnionTagTryFromError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(
            f,
            "attempted to turn {} into {}",
            self.0,
            stringify!(InOutUnionTag)
        )
    }
}
impl ::core::convert::TryFrom<usize> for InOutUnionTag {
    type Error = InOutUnionTagTryFromError;
    fn try_from(value: usize) -> ::std::result::Result<Self, Self::Error> {
        match value {
            x if x == Self::Producer as usize => Ok(InOutUnionTag::Producer),
            x if x == Self::Transformer as usize => Ok(InOutUnionTag::Transformer),
            x if x == Self::Consumer as usize => Ok(InOutUnionTag::Consumer),
            x => Err(InOutUnionTagTryFromError(x)),
        }
    }
}
impl ::fastfifo::config::FifoTag for InOutUnionTag {
    fn is_atomic(self) -> bool {
        match self {
            Self::Producer => false,
            Self::Transformer => true,
            Self::Consumer => false,
        }
    }
    fn chases(self) -> Self {
        match self {
            Self::Producer => Self::Consumer,
            Self::Transformer => Self::Producer,
            Self::Consumer => Self::Transformer,
        }
    }
    fn producer() -> Self {
        Self::Producer
    }
    fn num_transformations() -> usize {
        3usize
    }
}
impl<Input, Output> ::fastfifo::config::IndexedDrop<InOutUnionTag> for InOutUnion<Input, Output> {
    unsafe fn tagged_drop(&mut self, tag: InOutUnionTag) {
        match tag {
            InOutUnionTag::Producer => unsafe {
                ::core::mem::ManuallyDrop::drop(&mut self.producer)
            },
            InOutUnionTag::Transformer => unsafe {
                ::core::mem::ManuallyDrop::drop(&mut self.transformer)
            },
            InOutUnionTag::Consumer => unsafe {
                ::core::mem::ManuallyDrop::drop(&mut self.consumer)
            },
        }
    }
}
pub struct InOutUnionFifo<Input, Output>(
    ::fastfifo::FastFifo<InOutUnionTag, InOutUnion<Input, Output>>,
);
impl<Input, Output> ::fastfifo::config::TaggedClone<InOutUnionTag>
    for InOutUnionFifo<Input, Output>
{
    fn unchecked_clone(&self) -> Self {
        Self(self.0.unchecked_clone())
    }
}
impl<Input, Output> InOutUnionFifo<Input, Output> {
    #[allow(dead_code)]
    pub fn new(num_blocks: usize, block_size: usize) -> Self {
        Self(::fastfifo::FastFifo::new(num_blocks, block_size))
    }
}
impl<Input, Output> InOutUnionFifo<Input, Output> {
    #[allow(dead_code)]
    pub fn get_entry(
        &self,
        tag: InOutUnionTag,
    ) -> ::fastfifo::Result<
        ::fastfifo::entry_descriptor::EntryDescriptor<'_, InOutUnionTag, InOutUnion<Input, Output>>,
    > {
        self.0.get_entry(tag)
    }
    #[allow(dead_code)]
    pub fn split(
        self,
    ) -> (
        InOutUnionProducerFifo<Input, Output>,
        InOutUnionTransformerFifo<Input, Output>,
        InOutUnionConsumerFifo<Input, Output>,
    ) {
        (
            InOutUnionProducerFifo(
                <Self as ::fastfifo::config::TaggedClone<InOutUnionTag>>::unchecked_clone(&self),
            ),
            InOutUnionTransformerFifo(
                <Self as ::fastfifo::config::TaggedClone<InOutUnionTag>>::unchecked_clone(&self),
            ),
            InOutUnionConsumerFifo(
                <Self as ::fastfifo::config::TaggedClone<InOutUnionTag>>::unchecked_clone(&self),
            ),
        )
    }
}
pub struct InOutUnionProducerEntry<'entry_descriptor_lifetime, Input, Output>(
    ::fastfifo::entry_descriptor::EntryDescriptor<
        'entry_descriptor_lifetime,
        InOutUnionTag,
        InOutUnion<Input, Output>,
    >,
);
impl<'entry_descriptor_lifetime, Input, Output>
    From<
        ::fastfifo::entry_descriptor::EntryDescriptor<
            'entry_descriptor_lifetime,
            InOutUnionTag,
            InOutUnion<Input, Output>,
        >,
    > for InOutUnionProducerEntry<'entry_descriptor_lifetime, Input, Output>
{
    fn from(
        value: ::fastfifo::entry_descriptor::EntryDescriptor<
            'entry_descriptor_lifetime,
            InOutUnionTag,
            InOutUnion<Input, Output>,
        >,
    ) -> Self {
        Self(value)
    }
}
impl<'entry_descriptor_lifetime, Input, Output>
    Into<
        ::fastfifo::entry_descriptor::EntryDescriptor<
            'entry_descriptor_lifetime,
            InOutUnionTag,
            InOutUnion<Input, Output>,
        >,
    > for InOutUnionProducerEntry<'entry_descriptor_lifetime, Input, Output>
{
    fn into(
        self,
    ) -> ::fastfifo::entry_descriptor::EntryDescriptor<
        'entry_descriptor_lifetime,
        InOutUnionTag,
        InOutUnion<Input, Output>,
    > {
        self.0
    }
}
impl<'entry_descriptor_lifetime, Input, Output>
    InOutUnionProducerEntry<'entry_descriptor_lifetime, Input, Output>
{
    #[allow(dead_code)]
    pub fn transform<F: ::std::ops::FnOnce() -> Input>(&mut self, transformer: F) {
        self.0.modify_t_in_place(|ptr| unsafe {
            ptr.write(InOutUnion {
                producer: ::core::mem::ManuallyDrop::new(transformer()),
            })
        })
    }
}
pub struct InOutUnionProducerFifo<Input, Output>(InOutUnionFifo<Input, Output>);
impl<Input, Output> ::fastfifo::config::TaggedClone<InOutUnionTag>
    for InOutUnionProducerFifo<Input, Output>
{
    fn unchecked_clone(&self) -> Self {
        Self(self.0.unchecked_clone())
    }
}
impl<Input, Output> Clone for InOutUnionProducerFifo<Input, Output> {
    fn clone(&self) -> Self {
        <Self as ::fastfifo::config::TaggedClone<InOutUnionTag>>::tagged_clone(
            &self,
            InOutUnionTag::Producer,
        )
        .expect("this variant was marked with `atomic = false` and cannot be cloned")
    }
}
impl<Input, Output> InOutUnionProducerFifo<Input, Output> {
    #[allow(dead_code)]
    pub fn get_entry<'entry_descriptor_lifetime>(
        &'entry_descriptor_lifetime self,
    ) -> ::fastfifo::Result<InOutUnionProducerEntry<'entry_descriptor_lifetime, Input, Output>>
    {
        self.0
            .get_entry(InOutUnionTag::Producer)
            .map(InOutUnionProducerEntry::from)
    }
    #[allow(dead_code)]
    pub fn transform<F: ::std::ops::FnOnce() -> Input>(
        &self,
        transformer: F,
    ) -> ::fastfifo::Result<()> {
        self.get_entry()
            .map(|mut entry| entry.transform(transformer))
    }
}
pub struct InOutUnionTransformerEntry<'entry_descriptor_lifetime, Input, Output>(
    ::fastfifo::entry_descriptor::EntryDescriptor<
        'entry_descriptor_lifetime,
        InOutUnionTag,
        InOutUnion<Input, Output>,
    >,
);
impl<'entry_descriptor_lifetime, Input, Output>
    From<
        ::fastfifo::entry_descriptor::EntryDescriptor<
            'entry_descriptor_lifetime,
            InOutUnionTag,
            InOutUnion<Input, Output>,
        >,
    > for InOutUnionTransformerEntry<'entry_descriptor_lifetime, Input, Output>
{
    fn from(
        value: ::fastfifo::entry_descriptor::EntryDescriptor<
            'entry_descriptor_lifetime,
            InOutUnionTag,
            InOutUnion<Input, Output>,
        >,
    ) -> Self {
        Self(value)
    }
}
impl<'entry_descriptor_lifetime, Input, Output>
    Into<
        ::fastfifo::entry_descriptor::EntryDescriptor<
            'entry_descriptor_lifetime,
            InOutUnionTag,
            InOutUnion<Input, Output>,
        >,
    > for InOutUnionTransformerEntry<'entry_descriptor_lifetime, Input, Output>
{
    fn into(
        self,
    ) -> ::fastfifo::entry_descriptor::EntryDescriptor<
        'entry_descriptor_lifetime,
        InOutUnionTag,
        InOutUnion<Input, Output>,
    > {
        self.0
    }
}
impl<'entry_descriptor_lifetime, Input, Output>
    InOutUnionTransformerEntry<'entry_descriptor_lifetime, Input, Output>
{
    #[allow(dead_code)]
    pub fn transform<F: ::std::ops::FnOnce(Input) -> Output>(&mut self, transformer: F) {
        self.0.modify_t_in_place(|ptr| unsafe {
            ptr.write(InOutUnion {
                transformer: ::core::mem::ManuallyDrop::new(transformer(
                    <::core::mem::ManuallyDrop<Input>>::into_inner(ptr.read().producer),
                )),
            })
        })
    }
}
pub struct InOutUnionTransformerFifo<Input, Output>(InOutUnionFifo<Input, Output>);
impl<Input, Output> ::fastfifo::config::TaggedClone<InOutUnionTag>
    for InOutUnionTransformerFifo<Input, Output>
{
    fn unchecked_clone(&self) -> Self {
        Self(self.0.unchecked_clone())
    }
}
impl<Input, Output> Clone for InOutUnionTransformerFifo<Input, Output> {
    fn clone(&self) -> Self {
        <Self as ::fastfifo::config::TaggedClone<InOutUnionTag>>::tagged_clone(
            &self,
            InOutUnionTag::Transformer,
        )
        .expect("this variant was marked with `atomic = false` and cannot be cloned")
    }
}
impl<Input, Output> InOutUnionTransformerFifo<Input, Output> {
    #[allow(dead_code)]
    pub fn get_entry<'entry_descriptor_lifetime>(
        &'entry_descriptor_lifetime self,
    ) -> ::fastfifo::Result<InOutUnionTransformerEntry<'entry_descriptor_lifetime, Input, Output>>
    {
        self.0
            .get_entry(InOutUnionTag::Transformer)
            .map(InOutUnionTransformerEntry::from)
    }
    #[allow(dead_code)]
    pub fn transform<F: ::std::ops::FnOnce(Input) -> Output>(
        &self,
        transformer: F,
    ) -> ::fastfifo::Result<()> {
        self.get_entry()
            .map(|mut entry| entry.transform(transformer))
    }
}
pub struct InOutUnionConsumerEntry<'entry_descriptor_lifetime, Input, Output>(
    ::fastfifo::entry_descriptor::EntryDescriptor<
        'entry_descriptor_lifetime,
        InOutUnionTag,
        InOutUnion<Input, Output>,
    >,
);
impl<'entry_descriptor_lifetime, Input, Output>
    From<
        ::fastfifo::entry_descriptor::EntryDescriptor<
            'entry_descriptor_lifetime,
            InOutUnionTag,
            InOutUnion<Input, Output>,
        >,
    > for InOutUnionConsumerEntry<'entry_descriptor_lifetime, Input, Output>
{
    fn from(
        value: ::fastfifo::entry_descriptor::EntryDescriptor<
            'entry_descriptor_lifetime,
            InOutUnionTag,
            InOutUnion<Input, Output>,
        >,
    ) -> Self {
        Self(value)
    }
}
impl<'entry_descriptor_lifetime, Input, Output>
    Into<
        ::fastfifo::entry_descriptor::EntryDescriptor<
            'entry_descriptor_lifetime,
            InOutUnionTag,
            InOutUnion<Input, Output>,
        >,
    > for InOutUnionConsumerEntry<'entry_descriptor_lifetime, Input, Output>
{
    fn into(
        self,
    ) -> ::fastfifo::entry_descriptor::EntryDescriptor<
        'entry_descriptor_lifetime,
        InOutUnionTag,
        InOutUnion<Input, Output>,
    > {
        self.0
    }
}
impl<'entry_descriptor_lifetime, Input, Output>
    InOutUnionConsumerEntry<'entry_descriptor_lifetime, Input, Output>
{
    #[allow(dead_code)]
    pub fn transform<F: ::std::ops::FnOnce(Output)>(&mut self, transformer: F) {
        self.0.modify_t_in_place(|ptr| unsafe {
            transformer(<::core::mem::ManuallyDrop<Output>>::into_inner(
                ptr.read().transformer,
            ))
        })
    }
}
pub struct InOutUnionConsumerFifo<Input, Output>(InOutUnionFifo<Input, Output>);
impl<Input, Output> ::fastfifo::config::TaggedClone<InOutUnionTag>
    for InOutUnionConsumerFifo<Input, Output>
{
    fn unchecked_clone(&self) -> Self {
        Self(self.0.unchecked_clone())
    }
}
impl<Input, Output> Clone for InOutUnionConsumerFifo<Input, Output> {
    fn clone(&self) -> Self {
        <Self as ::fastfifo::config::TaggedClone<InOutUnionTag>>::tagged_clone(
            &self,
            InOutUnionTag::Consumer,
        )
        .expect("this variant was marked with `atomic = false` and cannot be cloned")
    }
}
impl<Input, Output> InOutUnionConsumerFifo<Input, Output> {
    #[allow(dead_code)]
    pub fn get_entry<'entry_descriptor_lifetime>(
        &'entry_descriptor_lifetime self,
    ) -> ::fastfifo::Result<InOutUnionConsumerEntry<'entry_descriptor_lifetime, Input, Output>>
    {
        self.0
            .get_entry(InOutUnionTag::Consumer)
            .map(InOutUnionConsumerEntry::from)
    }
    #[allow(dead_code)]
    pub fn transform<F: ::std::ops::FnOnce(Output)>(
        &self,
        transformer: F,
    ) -> ::fastfifo::Result<()> {
        self.get_entry()
            .map(|mut entry| entry.transform(transformer))
    }
}

// To see the timings of the fastfifo library, use feature "debug"
// RUST_LOG=fastfifo=info cargo run --release --bin varaidic_perf -F cli,debug -- -n 4 -b 100 -o 100
// to see these timings along with those in main
// RUST_LOG=info cargo run --release --bin varaidic_perf -F cli,debug -- -n 4 -b 100 -o 100

// To see the timings of main
// RUST_LOG=variadic_perf=info cargo run --release --bin variadic_perf -F cli -- -n 4 -b 100 -o 100
// To see thread timings (slows library)
// RUST_LOG=variadic_perf=info cargo run --release --bin variadic_perf -F cli,debug -- -n 4 -b 100 -o 100

// To see no timing info at all (this is not recommended)
// cargo run --release --bin variadic_perf -F cli -- -n 4 -b 100 -o 100

// With example options
// RUST_LOG=fastfifo=info cargo run --release --bin varaidic_perf -F cli,debug -- -n 100 -b 1000 -t 10 -o 100 -l out.log

fn main() {
    let Cli {
        nops,
        num_trans_threads,
        log_file,
        block_size,
        num_blocks,
    } = Cli::parse();

    let log_file = File::create(log_file.unwrap_or("variadic_perf.log".to_string())).unwrap();
    let (non_blocking_writer, _guard) = non_blocking(log_file);

    let file_layer = layer().with_writer(non_blocking_writer).with_ansi(false);

    Registry::default()
        .with(file_layer)
        .with(EnvFilter::from_default_env())
        .init();

    let num_trans_threads = num_trans_threads.unwrap_or(1);

    let epoch = Instant::now();
    let deadline = epoch + Duration::from_millis(100);

    let fifo = InOutUnionFifo::<usize, usize>::new(num_blocks, block_size);

    let (producer, transformer, consumer) = fifo.split();

    let producing_thread = {
        let deadline = deadline.clone();
        let fifo = producer;

        std::thread::spawn(move || {
            let span = span!(Level::INFO, "producer");
            let _guard = span.enter();

            sleep_until(deadline);

            info!("Woken");

            for i in 0..nops {
                while fifo
                    .transform(|| {
                        #[cfg(feature = "debug")]
                        info!("Op {i}: Uninit -> {i}");
                        i
                    })
                    .is_err()
                {
                    // sleep(Duration::from_millis(100));
                    std::hint::spin_loop();
                }
            }

            info!("Done");
        })
    };

    info!("Created prod thread");

    let mut trans_threads = Vec::with_capacity(num_trans_threads);
    for _ in 0..num_trans_threads {
        let fifo = transformer.clone();
        let deadline = deadline.clone();

        trans_threads.push(std::thread::spawn(move || {
            let span = span!(Level::INFO, "transformer");
            let _guard = span.enter();

            sleep_until(deadline); // + Duration::from_millis(1000));

            info!("Woken");

            for i in 0..nops {
                while fifo
                    .transform(|input| {
                        #[cfg(feature = "debug")]
                        info!("Op {i}: {input} -> {}", input + 1);
                        input + 1
                    })
                    .is_err()
                {
                    // sleep(Duration::from_millis(100));
                    std::hint::spin_loop();
                }
                let _ = i;
            }

            info!("Done");
        }))
    }

    info!("Created transformer threads");

    let consuming_thread = {
        let fifo = consumer;
        let deadline = deadline.clone();

        std::thread::spawn(move || {
            let span = span!(Level::INFO, "consumer");
            let _guard = span.enter();

            sleep_until(deadline); // + Duration::from_millis(2000));

            info!("Woken");

            for i in 0..nops {
                while fifo
                    .transform(|output| {
                        #[cfg(feature = "debug")]
                        info!("Op {i}: {output} -> Uninit");
                        #[cfg(feature = "debug")]
                        if output != i + 1 {
                            error!("FAILED ASSERTION `output ({output}) == i + 1 ({})`", i + 1);
                        } else {
                            info!("SUCCEEDED ASSERTION `output == i + 1` ({output})")
                        }
                        assert_eq!(output, i + 1)
                    })
                    .is_err()
                {
                    // sleep(Duration::from_millis(100));
                    std::hint::spin_loop();
                }
            }

            info!("Done");
        })
    };

    info!("Created cons thread");

    sleep_until(deadline);

    info!("Woken from sleep");

    consuming_thread.join().unwrap();
    trans_threads.into_iter().for_each(|t| t.join().unwrap());
    producing_thread.join().unwrap();

    info!("Threads joined");

    info!(
        "Estimated rate ({:.2e} ops/s)",
        nops as f64 / deadline.elapsed().as_secs_f64()
    );
}
