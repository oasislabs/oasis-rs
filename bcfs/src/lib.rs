#![feature(cell_update)]

#[cfg(feature = "ffi")]
pub mod ffi;

type Result<T> = std::result::Result<T, wasi_types::ErrNo>;

mod bcfs;
mod file;

pub use crate::bcfs::BCFS;

pub enum AnyAddress<A: blockchain_traits::Address> {
    Native(A),
    Foreign(String),
}

#[cfg(test)]
mod tests;
