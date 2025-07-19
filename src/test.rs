#[cfg(test)]
use crate::FastFifo;
use std::array;
#[cfg(test)]
use std::sync::Arc;
use std::thread::JoinHandle;
#[cfg(test)]
use std::time::{Duration, Instant};

#[test]
fn test_construct() {
    let _ = FastFifo::<u64, 10, 10>::new();
}

#[test]
fn test_push_pop() {
    const NUM_BLOCKS: usize = 4;
    const BLOCK_SIZE: usize = 10;

    let fifo = FastFifo::<usize, NUM_BLOCKS, BLOCK_SIZE>::new();

    println!("Empty fifo: {fifo:?}\n");

    for i in 0..100 {
        for j in 0..(NUM_BLOCKS * BLOCK_SIZE / 3) {
            fifo.push(i + j).unwrap();
        }
        println!("{fifo:?}\n");
        for j in 0..(NUM_BLOCKS * BLOCK_SIZE / 3) {
            assert!(j + i == fifo.pop().unwrap());
        }
    }
}

#[test]
fn multi_thread() {
    const NUM_BLOCKS: usize = 12;
    const BLOCK_SIZE: usize = 30;
    const THREAD_COUNT: usize = 12;

    type Fifo = Arc<FastFifo<usize, NUM_BLOCKS, BLOCK_SIZE>>;

    fn gen_thread_task(fifo: Fifo, deadline: Instant) -> impl Fn() {
        move || {
            std::thread::sleep_until(deadline);

            for i in 0..25 {
                fifo.push(i).unwrap();
            }
            // for i in 0..25 {
            //     fifo.pop().unwrap();
            // }
        }
    }

    let fifo: Fifo = Arc::new(FastFifo::new());

    let deadline = Instant::now() + Duration::from_millis(500);

    let threads: [JoinHandle<()>; THREAD_COUNT] =
        array::from_fn(|_| std::thread::spawn(gen_thread_task(fifo.clone(), deadline.clone())));

    threads
        .into_iter()
        .for_each(|handle| handle.join().unwrap());

    println!("{fifo:?}")
}
