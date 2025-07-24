#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use fastfifoprocmacro::generate_union;
use std::{thread::sleep, time::Duration};

fn main() {
    generate_union! {
        pub InOutUnion<Input, Output> {
            Producer: Input, atomic = false;
            Transformer: Output, atomic = true;
            Consumer: (), atomic = false;
        }
    }

    let fifo = InOutUnionFifo::<usize, usize, 10, 10>::new();
    let (producer, transformer, consumer) = fifo.split();

    let producing_thread = std::thread::spawn(move || {
        for i in 0..10 {
            producer.transform(|()| i).unwrap()
        }
    });

    let transforming_thread = std::thread::spawn(move || {
        sleep(Duration::from_millis(10));

        for i in 0..10 {
            transformer.transform(|input| input + 1).unwrap();
        }
    });

    let consuming_thread = std::thread::spawn(move || {
        sleep(Duration::from_millis(20));

        for i in 0..10 {
            consumer
                .transform(|output| assert_eq!(output, i + 1))
                .unwrap();
        }
    });

    producing_thread.join().unwrap();
    transforming_thread.join().unwrap();
    consuming_thread.join().unwrap();
}
