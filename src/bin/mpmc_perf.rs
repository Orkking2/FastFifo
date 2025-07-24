#![feature(thread_sleep_until)]

use fastfifo::mpmc::FastFifo;
use std::{
    array,
    thread::{self, JoinHandle, sleep_until},
    time::{Duration, Instant},
};

// cargo run --release --bin mpmc_perf
fn main() {
    const NUM_PROD_THREADS: usize = 1;
    const NUM_CONS_THREADS: usize = 1;

    const NUM_PUSH_POP: usize = 1_000_000_000;

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

    let prod_threads: [JoinHandle<()>; NUM_PROD_THREADS] = array::from_fn(|_| {
        let fifo = fifo.clone();
        let deadline = deadline.clone();
        thread::spawn(move || {
            sleep_until(deadline);

            for i in 0..NUM_PUSH_POP {
                while fifo.push(i).is_err() {
                    std::hint::spin_loop();
                }
            }
        })
    });

    println!("Created prod threads ({:?})", epoch.elapsed());

    let cons_threads: [JoinHandle<()>; NUM_CONS_THREADS] = array::from_fn(|_| {
        let fifo = fifo.clone();
        let deadline = deadline.clone();
        thread::spawn(move || {
            sleep_until(deadline);

            for _ in 0..NUM_PUSH_POP {
                while fifo.pop().is_err() {
                    std::hint::spin_loop();
                }
            }
        })
    });

    println!("Created cons threads ({:?})", epoch.elapsed());

    sleep_until(deadline);

    println!("Woken from sleep_until ({:?})", epoch.elapsed());

    prod_threads.into_iter().for_each(|t| t.join().unwrap());

    println!("Prod threads joined ({:?})", epoch.elapsed());

    cons_threads.into_iter().for_each(|t| t.join().unwrap());

    println!("Cons threads joined ({:?})", epoch.elapsed());

    println!(
        "Estimated rate ({:.2e} ops/s)",
        NUM_PUSH_POP as f64 * (NUM_CONS_THREADS as f64 + NUM_CONS_THREADS as f64)
            / epoch.elapsed().as_secs_f64()
    );
}
