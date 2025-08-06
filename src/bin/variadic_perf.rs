#![feature(thread_sleep_until)]
#![feature(allocator_api)]

use clap::Parser;
use fastfifo::generate_union;
use std::{
    fs::File,
    thread::sleep_until,
    time::{Duration, Instant},
};
use tracing::{error, info, instrument, span, Level};
use tracing_appender::non_blocking;
use tracing_subscriber::{
    Registry,
    fmt::layer,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

#[derive(Parser, Debug)]
struct Cli {
    #[arg(short = 'o', long)]
    nops: Option<usize>,

    #[arg(short = 't', long)]
    num_trans_threads: Option<usize>,
}

#[instrument]
fn main() {
    let file = File::create("variadic_perf.log").unwrap();
    let (non_blocking_writer, _guard) = non_blocking(file);

    let file_layer = layer().with_writer(non_blocking_writer).with_ansi(false);

    Registry::default().with(file_layer).init();

    generate_union! {
        pub InOutUnion<Input, Output> {
            Producer: Input, atomic = false;
            Transformer: Output, atomic = true;
            Consumer: (), atomic = false;
        }
    }

    let Cli {
        nops,
        num_trans_threads,
    } = Cli::parse();

    let nops = nops.unwrap_or(1_000_000_000);
    let num_trans_threads = num_trans_threads.unwrap_or(1);

    let epoch = Instant::now();
    let deadline = epoch + Duration::from_millis(100);

    let fifo = InOutUnionFifo::<usize, usize>::new(4, 10);
    let (producer, transformer, consumer) = fifo.split();

    let producing_thread = {
        let deadline = deadline.clone();
        let fifo = producer;

        std::thread::spawn(move || {
            let span = span!(Level::INFO, "producer");

            sleep_until(deadline);

            info!(parent: &span, "Woken");

            for i in 0..nops {
                while fifo
                    .transform(|| {
                        info!(parent: &span, "Op {i}: Uninit -> {i}");
                        i
                    })
                    .is_err()
                {
                    // sleep(Duration::from_millis(100));
                    std::hint::spin_loop();
                }
            }

            info!(parent: &span, "Done");
        })
    };

    info!("Created prod thread ({:?})", epoch.elapsed());

    let mut trans_threads = Vec::with_capacity(num_trans_threads);
    for _ in 0..num_trans_threads {
        let fifo = transformer.clone();
        let deadline = deadline.clone();

        trans_threads.push(std::thread::spawn(move || {
            let span = span!(Level::INFO, "transformer");

            sleep_until(deadline); // + Duration::from_millis(1000));

            info!(parent: &span, "Woken");

            for i in 0..nops {
                while fifo
                    .transform(|input| {
                        info!(parent: &span, "Op {i}: {input} -> {}", input + 1);
                        input + 1
                    })
                    .is_err()
                {
                    // sleep(Duration::from_millis(100));
                    std::hint::spin_loop();
                }
            }

            info!(parent: &span, "done");
        }))
    }

    info!("Created trans threads ({:?})", epoch.elapsed());

    let consuming_thread = {
        let fifo = consumer;
        let deadline = deadline.clone();

        std::thread::spawn(move || {
            let span = span!(Level::INFO, "consumer");

            sleep_until(deadline); // + Duration::from_millis(2000));

            info!(parent: &span, "Woken");

            for i in 0..nops {
                while fifo
                    .transform(|output| {
                        info!(parent: &span, "Op {i}: {output} -> Uninit");
                        if output != i + 1 {
                            error!(parent: &span, "FAILED ASSERTION `output ({output}) == i + 1 ({})`", i + 1);
                        } else {
                            info!(parent: &span, "SUCCEEDED ASSERTION `output == i + 1` ({output})")
                        }
                        assert_eq!(output, i + 1)
                    })
                    .is_err()
                {
                    // sleep(Duration::from_millis(100));
                    std::hint::spin_loop();
                }
            }

            info!(parent: &span, "done");
        })
    };

    info!("Created cons thread ({:?})", epoch.elapsed());

    sleep_until(deadline);

    info!("Woken from sleep ({:?})", epoch.elapsed());

    consuming_thread.join().unwrap();
    trans_threads.into_iter().for_each(|t| t.join().unwrap());
    producing_thread.join().unwrap();

    info!("Threads joined ({:?})", epoch.elapsed());

    info!(
        "Estimated rate ({:.2e} ops/s)",
        nops as f64 / deadline.elapsed().as_secs_f64()
    )
}
