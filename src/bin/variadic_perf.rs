#![feature(thread_sleep_until)]

use clap::Parser;
use fastfifo::generate_union;
use std::{
    thread::{sleep, sleep_until},
    time::{Duration, Instant},
};

#[derive(Parser, Debug)]
struct Cli {
    #[arg(short = 'o', long)]
    nops: Option<usize>,

    #[arg(short = 't', long)]
    num_trans_threads: Option<usize>,
}

fn main() {
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

    let fifo = InOutUnionFifo::<usize, usize, 10, 10_000>::new();
    let (producer, transformer, consumer) = fifo.split();

    let producing_thread = {
        let deadline = deadline.clone();
        let fifo = producer;

        std::thread::spawn(move || {
            sleep_until(deadline);

            for i in 0..nops {
                while fifo.transform(|| i).is_err() {
                    std::hint::spin_loop();
                }
            }
        })
    };

    println!("Created prod thread ({:?})", epoch.elapsed());

    let mut trans_threads = Vec::with_capacity(num_trans_threads);
    for _ in 0..num_trans_threads {
        let fifo = transformer.clone();
        let deadline = deadline.clone();

        trans_threads.push(std::thread::spawn(move || {
            sleep_until(deadline);

            for _ in 0..nops {
                while fifo.transform(|input| input + 1).is_err() {
                    std::hint::spin_loop();
                }
            }
        }))
    }

    println!("Created trans threads ({:?})", epoch.elapsed());

    let consuming_thread = {
        let fifo = consumer;
        let deadline = deadline.clone();

        std::thread::spawn(move || {
            sleep_until(deadline);

            for i in 0..nops {
                while fifo.transform(|output| assert_eq!(output, i + 1)).is_err() {
                    std::hint::spin_loop();
                }
            }
        })
    };

    println!("Created cons thread ({:?})", epoch.elapsed());

    sleep_until(deadline);

    println!("Woken from sleep until ({:?})", epoch.elapsed());

    producing_thread.join().unwrap();
    trans_threads.into_iter().for_each(|t| t.join().unwrap());
    consuming_thread.join().unwrap();

    println!("Threads joined ({:?})", epoch.elapsed());

    println!(
        "Estimated rate ({:.2e} ops/s)",
        nops as f64 / deadline.elapsed().as_secs_f64()
    )
}
