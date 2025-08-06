#![feature(thread_sleep_until)]
#![feature(allocator_api)]

use clap::Parser;
use fastfifo::generate_union;
use std::{
    fs::File,
    thread::sleep_until,
    time::{Duration, Instant},
};
use tracing::{Level, error, info, span};
use tracing_appender::non_blocking;
use tracing_subscriber::{
    EnvFilter, Registry, fmt::layer, layer::SubscriberExt, util::SubscriberInitExt,
};

#[derive(Parser, Debug)]
struct Cli {
    #[arg(short = 'o', long)]
    nops: Option<usize>,

    #[arg(short = 't', long)]
    num_trans_threads: Option<usize>,

    #[arg(short = 'l', long)]
    log_file: Option<String>,
}

generate_union! {
    pub InOutUnion<Input, Output> {
        Producer: Input, atomic = false;
        Transformer: Output, atomic = true;
        Consumer: (), atomic = false;
    }
}

// To see the timings of the fastfifo library, use feature "debug"
// RUST_LOG=fastfifo=info cargo run --release --bin varaidic_perf -F cli,debug -- -o 100
// to see these timings along with those in main
// RUST_LOG=info cargo run --release --bin varaidic_perf -F cli,debug -- -o 100

// To see the timings of main
// RUST_LOG=variadic_perf=info cargo run --release --bin variadic_perf -F cli -- -o 100
// To see thread timings (slows library)
// RUST_LOG=variadic_perf=info cargo run --release --bin variadic_perf -F cli,debug -- -o 100

// To see no timing info at all (this is not recommended)
// cargo run --release --bin variadic_perf -F cli -- -o 100

// With example options
// RUST_LOG=fastfifo=info cargo run --release --bin varaidic_perf -F cli,debug -- -t 10 -o 100 -l out.log

fn main() {
    let Cli {
        nops,
        num_trans_threads,
        log_file,
    } = Cli::parse();

    let log_file = File::create(log_file.unwrap_or("variadic_perf.log".to_string())).unwrap();
    let (non_blocking_writer, _guard) = non_blocking(log_file);

    let file_layer = layer().with_writer(non_blocking_writer).with_ansi(false);

    Registry::default()
        .with(file_layer)
        .with(EnvFilter::from_default_env())
        .init();

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

    info!("Estimated rate ({:.2e} ops/s)", nops as f64 / deadline.elapsed().as_secs_f64())
}
