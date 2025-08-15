#![cfg(loom)]

use fastfifo::generate_union;
use loom::cell::UnsafeCell;
use loom::sync::Arc;
use loom::sync::atomic::{
    AtomicUsize,
    Ordering::{Acquire, Relaxed, Release},
};
use loom::thread;
use std::mem::MaybeUninit;

// 0 = empty, 1 = full
struct Slot<T> {
    state: AtomicUsize,
    data: UnsafeCell<MaybeUninit<T>>,
}

impl<T> Slot<T> {
    fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

generate_union! {
    pub InOutUnion<T, U> {
        Producer: T, atomic = true;
        Transformer: U, atomic = true;
        Consumer: (), atomic = true;
    }
}

// -n 128 -b 1000000 -o 1000000 -t 8

generate_union! {
    pub SimpleUnion<T> {
        Producer: T, atomic = true;
        Consumer: (), atomic = true;
    }
}

#[test]
fn simple_publishing_test() {
    loom::model(|| {
        let (prod, cons) = SimpleUnionFifo::<usize>::new(100, 100).split();

        let p = {
            thread::spawn(move || {
                while prod.transform(|| 1).is_err() {
                    loom::hint::spin_loop();
                    loom::thread::yield_now();
                }
            })
        };

        let c = {
            thread::spawn(move || {
                while cons.transform(|cont| assert_eq!(cont, 1)).is_err() {
                    loom::hint::spin_loop();
                    loom::thread::yield_now();
                }
            })
        };

        p.join().unwrap();
        c.join().unwrap();
    })
}

#[test]
fn full_publishing_test() {
    loom::model(|| {
        let (producer, transformer, consumer) = InOutUnionFifo::<usize, usize>::new(2, 10).split();

        let p = {
            thread::spawn(move || {
                while producer.transform(|| 1).is_err() {
                    loom::hint::spin_loop();
                    loom::thread::yield_now();
                }
            })
        };

        let t = {
            thread::spawn(move || {
                while transformer.transform(|i| i + 1).is_err() {
                    loom::hint::spin_loop();
                    loom::thread::yield_now();
                }
            })
        };

        let c = {
            thread::spawn(move || {
                while consumer.transform(|i| assert_eq!(i, 2)).is_err() {
                    loom::hint::spin_loop();
                    loom::thread::yield_now();
                }
            })
        };

        p.join().unwrap();
        t.join().unwrap();
        c.join().unwrap();
    });
}

#[test]
fn publish_then_consume_is_visible() {
    loom::model(|| {
        let slot = Arc::new(Slot::<u64>::new());

        let p = {
            let slot = slot.clone();
            thread::spawn(move || {
                // Producer writes all fields, then publishes with Release.
                unsafe {
                    slot.data
                        .get_mut()
                        .deref()
                        .as_mut_ptr()
                        .write(0xDEAD_F00D_DEAD_BEEFu64);
                }
                slot.state.store(1, Release); // publish
            })
        };

        let c = {
            let slot = slot.clone();
            thread::spawn(move || {
                // Consumer must Acquire before reading fields.
                if slot.state.load(Acquire) == 1 {
                    let v = unsafe { slot.data.get().deref().assume_init_read() };
                    assert_eq!(v, 0xDEAD_F00D_DEAD_BEEFu64);
                }
            })
        };

        p.join().unwrap();
        c.join().unwrap();
    });
}

// This shows the bug Loom should catch if you publish or consume with Relaxed.
#[test]
#[should_panic]
fn relaxed_is_wrong() {
    loom::model(|| {
        let slot = Arc::new(Slot::<u64>::new());

        let p = {
            let slot = slot.clone();
            thread::spawn(move || {
                unsafe {
                    slot.data
                        .get_mut()
                        .deref()
                        .as_mut_ptr()
                        .write(0xDEAD_BEEFu64);
                }
                slot.state.store(1, Relaxed); // wrong
            })
        };

        let c = {
            let slot = slot.clone();
            thread::spawn(move || {
                if slot.state.load(Relaxed) == 1 {
                    // wrong
                    let v = unsafe { slot.data.get().deref().assume_init_read() };
                    // Loom should find an execution where this observes old data.
                    assert_eq!(v, 0xDEAD_BEEFu64);
                }
            })
        };

        p.join().unwrap();
        c.join().unwrap();
    });
}
