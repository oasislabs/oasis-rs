//! Log module

use crate::prelude::*;

/// As log trait for how primitive types are represented as indexed arguments
/// of the event log
pub trait AsLog {
    /// Convert type to hash representation for the event log.
    fn as_log(&self) -> H256;
}

macro_rules! impl_int_as_log {
    ( $($ty:ty),+ ) => {
        $(
            impl AsLog for $ty {
                fn as_log(&self) -> H256 {
                    let mut result = H256::zero();
                    let start_idx = 32 - std::mem::size_of::<$ty>();
                    result.as_mut()[start_idx..32].copy_from_slice(&self.to_be_bytes());
                    result
                }
            }
        )+
    }
}

impl_int_as_log!(u8, i8, u16, i16, u32, i32, u64, i64, usize, isize);

impl AsLog for bool {
    fn as_log(&self) -> H256 {
        let mut result = H256::zero();
        result.as_mut()[32] = if *self { 1 } else { 0 };
        result
    }
}

impl AsLog for U256 {
    fn as_log(&self) -> H256 {
        let mut result = H256::zero();
        self.to_big_endian(result.as_mut());
        result
    }
}

impl AsLog for H256 {
    fn as_log(&self) -> H256 {
        self.clone()
    }
}

impl AsLog for Address {
    fn as_log(&self) -> H256 {
        (*self).into()
    }
}
