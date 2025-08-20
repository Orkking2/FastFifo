// #![feature(allocator_api)]

extern crate self as fastfifo;

pub use fastfifoprocmacro::generate_union;
pub use crate::error::Error;

pub mod mpmc;
// pub mod two_buff;

pub mod config;
pub mod entry_descriptor;
pub mod error;
pub mod fifo;

pub type Result<T> = std::result::Result<T, Error>;

mod atom_pair;
mod block;
mod field;
mod fifo_inner;
mod head;