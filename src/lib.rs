#![feature(specialization)]
#![feature(unsafe_cell_access)]
// For cohort
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

pub use crate::error::Error;

mod atomic;
mod block;
mod error;
mod field;

pub mod entries;
pub mod mpmc;
pub mod transform;

pub type Result<T> = std::result::Result<T, Error>;
