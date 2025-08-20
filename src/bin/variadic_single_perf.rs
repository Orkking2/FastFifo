#![feature(thread_sleep_until)]
// #![feature(allocator_api)]

#[cfg(feature = "debug")]
use tracing::error;

use clap::Parser;
use fastfifo::generate_union;
use std::{
    fs::File,
    path::PathBuf,
    thread::{self, sleep_until},
    time::{Duration, Instant},
};
use tracing::info;
use tracing_appender::non_blocking::NonBlockingBuilder;
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

    #[arg(short = 'l', long)]
    log_file: Option<String>,
}

generate_union! {
    pub InOutUnion<Input, Output> {
        Producer: Input, atomic = false;
        Transformer: Output, atomic = false;
        Consumer: (), atomic = false;
    }
}

fn main() {
    let Cli {
        nops,
        log_file,
        block_size,
        num_blocks,
    } = Cli::parse();

    let log_path = PathBuf::new().join("logs").join(format!(
        "{}.log",
        log_file.unwrap_or("variadic_perf".to_string())
    ));

    let log_file = File::create(log_path).unwrap();

    let (non_blocking_writer, _guard) = NonBlockingBuilder::default()
        .buffered_lines_limit(100_000)
        .lossy(false)
        .finish(log_file);

    let file_layer = layer()
        .with_writer(non_blocking_writer)
        .with_ansi(false)
        .without_time()
        .with_thread_names(true);

    Registry::default()
        .with(file_layer)
        .with(EnvFilter::from_default_env())
        .init();

    let epoch = Instant::now();
    let deadline = epoch + Duration::from_millis(100);

    let fifo = InOutUnionFifo::<usize, usize>::new(num_blocks, block_size);

    let (producer, transformer, consumer) = fifo.split();

    let prod_thread = {
        thread::Builder::new()
            .name(format!("producer"))
            .spawn(move || {
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
            .unwrap()
    };

    info!("Created producer thread");

    let mut trans_threads = Vec::with_capacity(num_trans_threads);
    for t in 0..num_trans_threads {
        let fifo = transformer.clone();
        let deadline = deadline.clone();

        trans_threads.push(
            thread::Builder::new()
                .name(format!("transformer-{t}"))
                .spawn(move || {
                    // let span = span!(Level::INFO, "transformer");
                    // let _guard = span.enter();

                    sleep_until(deadline); // + Duration::from_millis(1000));

                    info!("Woken");

                    for i in 0..trans_nops {
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
                })
                .unwrap(),
        )
    }

    info!("Created {num_trans_threads} transformer thread(s)");

    let mut cons_threads = Vec::with_capacity(num_cons_threads);
    for c in 0..num_cons_threads {
        let fifo = consumer.clone();
        let deadline = deadline.clone();

        cons_threads.push(
            thread::Builder::new()
                .name(format!("consumer-{c}"))
                .spawn(move || {
                    // let span = span!(Level::INFO, "consumer");
                    // let _guard = span.enter();

                    sleep_until(deadline); // + Duration::from_millis(2000));

                    info!("Woken");

                    for i in 0..cons_nops {
                        while fifo
                            .transform(|output| {
                                #[cfg(feature = "debug")]
                                info!("Op {i}: {output} -> Uninit");
                                #[cfg(feature = "debug")]
                                if output != i + 1 {
                                    error!(
                                        "FAILED ASSERTION `output ({output}) == i + 1 ({})`",
                                        i + 1
                                    );
                                } else {
                                    info!("SUCCEEDED ASSERTION `output == i + 1` ({output})")
                                }
                                // assert_eq!(output, i + 1)
                            })
                            .is_err()
                        {
                            // sleep(Duration::from_millis(100));
                            std::hint::spin_loop();
                        }
                    }

                    info!("Done");
                })
                .unwrap(),
        )
    }

    info!("Created {num_cons_threads} consumer thread(s)");

    sleep_until(deadline);

    info!("Woken from sleep");

    prod_threads.into_iter().for_each(|t| t.join().unwrap());
    trans_threads.into_iter().for_each(|t| t.join().unwrap());
    cons_threads.into_iter().for_each(|t| t.join().unwrap());

    info!("Threads joined");

    info!(
        "Estimated rate ({:.2e} ops/s)",
        true_nops as f64 / deadline.elapsed().as_secs_f64()
    );
}
