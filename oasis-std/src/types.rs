pub use oasis_types::*;

pub trait AddressExt {
    fn transfer(&self, value: u64) -> Result<(), crate::errors::ExtCallError>;

    fn balance(&self) -> u64;
}

impl AddressExt for Address {
    fn transfer(&self, value: u64) -> Result<(), crate::errors::ExtCallError> {
        crate::ext::transfer(self, value.into())
    }

    fn balance(&self) -> u64 {
        crate::ext::balance(self)
    }
}
