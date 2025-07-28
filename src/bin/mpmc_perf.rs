#![feature(thread_sleep_until)]

use clap::Parser;
use fastfifo::mpmc::FastFifo;
use std::{
    thread::{self, sleep_until},
    time::{Duration, Instant},
};

#[derive(Parser, Debug)]
struct Cli {
    #[arg(short = 'p', long = "nprod")]
    nprod: usize,

    #[arg(short = 'c', long = "ncons")]
    ncons: usize,

    #[arg(short = 'o', long = "nops")]
    nops: Option<usize>,
}

// cargo run --release --bin mpmc_perf -- --help
fn main() {
    let Cli { nprod, ncons, nops } = Cli::parse();
    let nops = nops.unwrap_or(1_000_000_000);

    let epoch = Instant::now();

    type Fifo = FastFifo<usize, 10, 10_000>;

    println!("Epoch ({:?})", epoch.elapsed());

    let fifo = Fifo::new();

    println!("Created fifo ({:?})", epoch.elapsed());

    let deadline = epoch + Duration::from_millis(100);

    println!("Created deadline ({:?})", epoch.elapsed());

    // cargo run --release --bin mpmc_perf
    // NBLK = 100, BSZ = 100, NPT = NCT = 1
    // 54962870.06
    // NPT = NCT = 4
    // 13721228.27
    // 14156713.99
    // NBLK = 100, BSZ = 1000 + spin_loop()
    // 14500308.96
    // NBLK = 10, BSZ = 10_000
    // 13021192.92
    // NPT = NCT = 2
    // 45780210.22
    // NPT = NCT = 1
    // 51813049.68

    let mut prod_threads = Vec::with_capacity(nprod);
    for _ in 0..nprod {
        let fifo = fifo.clone();
        let deadline = deadline.clone();
        prod_threads.push(thread::spawn(move || {
            sleep_until(deadline);

            for i in 0..nops {
                while fifo.push(i).is_err() {
                    std::hint::spin_loop();
                }
            }
        }))
    }

    println!("Created prod threads ({:?})", epoch.elapsed());

    let mut cons_threads = Vec::with_capacity(ncons);
    for _ in 0..ncons {
        let fifo = fifo.clone();
        let deadline = deadline.clone();

        cons_threads.push(thread::spawn(move || {
            sleep_until(deadline);

            for _ in 0..nops {
                while fifo.pop().is_err() {
                    std::hint::spin_loop();
                }
            }
        }))
    }

    println!("Created cons threads ({:?})", epoch.elapsed());

    sleep_until(deadline);

    println!("Woken from sleep_until ({:?})", epoch.elapsed());

    prod_threads.into_iter().for_each(|t| t.join().unwrap());

    println!("Prod threads joined ({:?})", epoch.elapsed());

    cons_threads.into_iter().for_each(|t| t.join().unwrap());

    println!("Cons threads joined ({:?})", epoch.elapsed());

    println!(
        "Estimated rate ({:.2e} ops/s)",
        nops as f64 * (ncons as f64 + nprod as f64) / deadline.elapsed().as_secs_f64()
    );
}
