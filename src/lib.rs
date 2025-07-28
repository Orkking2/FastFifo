extern crate self as fastfifo;

pub use crate::error::Error;

mod atomic;
mod block;
mod error;
mod field;

pub mod entries;
pub mod mpmc;
pub mod transform;

pub type Result<T> = std::result::Result<T, Error>;

pub use fastfifoprocmacro::generate_union;
