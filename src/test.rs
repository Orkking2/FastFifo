use crate::FastFifo;
use std::array;
use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[test]
fn test_construct() {
    let _ = FastFifo::<u64, 10, 10>::new();
}

#[test]
fn test_push_pop() {
    const NUM_BLOCKS: usize = 4;
    const BLOCK_SIZE: usize = 10;

    type Fifo = FastFifo<usize, NUM_BLOCKS, BLOCK_SIZE>;

    let fifo = Fifo::new();

    println!("Empty fifo: {fifo:?}\n");

    for i in 0..100 {
        for j in 0..(Fifo::capacity() / 3) {
            fifo.push(i + j).unwrap();
        }
        println!("{fifo:?}\n");
        for j in 0..(Fifo::capacity() / 3) {
            let popped = fifo.pop().unwrap();
            assert!(j + i == popped, "Expected {} but got {}", j + i, popped);
        }
    }
}

#[test]
fn multi_thread() {
    let epoch = Instant::now();

    type T = usize;
    const NUM_BLOCKS: usize = 12;
    const BLOCK_SIZE: usize = 250;

    const THREAD_COUNT: usize = 12;

    type Fifo = FastFifo<T, NUM_BLOCKS, BLOCK_SIZE>;

    fn gen_prod_thread_task(fifo: Fifo) -> impl Fn() {
        move || {
            for i in 0..250 {
                fifo.push(i).unwrap();
            }
        }
    }

    fn gen_cons_thread_task(fifo: Fifo) -> impl Fn() {
        move || {
            for i in 0..250 {
                fifo.pop().unwrap();
            }
        }
    }

    let fifo: Fifo = FastFifo::new();

    println!(
        "Starting prod threads ({:?})",
        Instant::now().duration_since(epoch)
    );

    let threads: [JoinHandle<()>; THREAD_COUNT] =
        array::from_fn(|_| std::thread::spawn(gen_prod_thread_task(fifo.clone())));

    println!(
        "Joining prod threads ({:?})",
        Instant::now().duration_since(epoch)
    );

    threads
        .into_iter()
        .for_each(|handle| handle.join().unwrap());

    println!(
        "Prod threads joined ({:?})",
        Instant::now().duration_since(epoch)
    );

    println!("Full {fifo:?}");

    let epoch = Instant::now();

    println!(
        "Starting cons threads ({:?})",
        Instant::now().duration_since(epoch)
    );

    let threads: [JoinHandle<()>; THREAD_COUNT] =
        array::from_fn(|_| std::thread::spawn(gen_cons_thread_task(fifo.clone())));

    println!(
        "Joining cons threads ({:?})",
        Instant::now().duration_since(epoch)
    );

    threads
        .into_iter()
        .for_each(|handle| handle.join().unwrap());

    println!(
        "Cons threads joined ({:?})",
        Instant::now().duration_since(epoch)
    );

    println!("Empty {fifo:?}");
}


struct DropTester {
    pub inner: usize,
}

impl Drop for DropTester {
    fn drop(&mut self) {
        println!("Drop {}", self.inner);
    }
}

#[test]
fn drop_test() {
    let testers: [DropTester; 100] = array::from_fn(|i| DropTester { inner: i });

    let fifo = FastFifo::<_, 10, 10>::new();

    for t in testers {
        println!("Pushing {}", t.inner);
        fifo.push(t).unwrap();
    }

    println!("Manual dropping...");

    for _ in 0..50 {
        fifo.pop().unwrap();
    }

    println!("Autodropping...");
}