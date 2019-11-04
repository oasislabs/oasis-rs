//! # map_vec: Map and Set APIs backed by Vecs.
#![feature(drain_filter, shrink_to, try_reserve)]

pub mod map;
pub mod set;

pub use map::Map;
pub use set::Set;
