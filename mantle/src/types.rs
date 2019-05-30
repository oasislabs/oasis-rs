pub use mantle_types::*;

pub trait AddressExt {
    fn transfer<'a, V: Into<&'a U256>>(&self, value: V) -> Result<(), crate::errors::ExtCallError>;

    fn balance(&self) -> U256;
}

impl AddressExt for Address {
    fn transfer<'a, V: Into<&'a U256>>(&self, value: V) -> Result<(), crate::errors::ExtCallError> {
        crate::ext::transfer(self, value.into())
    }

    fn balance(&self) -> U256 {
        crate::ext::balance(self)
    }
}
