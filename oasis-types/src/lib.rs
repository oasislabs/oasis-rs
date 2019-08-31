#[macro_use]
extern crate serde;

mod address;

pub use address::Address;

#[repr(C)]
#[derive(PartialEq, Eq)]
#[doc(hidden)]
pub struct ExtStatusCode(pub u32);

#[allow(non_upper_case_globals)] // it's supposed to be a non-exhaustive enum
impl ExtStatusCode {
    pub const Success: ExtStatusCode = ExtStatusCode(0);
    pub const InsufficientFunds: ExtStatusCode = ExtStatusCode(1);
    pub const InvalidInput: ExtStatusCode = ExtStatusCode(2);
    pub const NoAccount: ExtStatusCode = ExtStatusCode(3);
}

#[derive(Clone, Default, Debug)]
pub struct AccountMeta {
    pub balance: u128,
    pub expiry: Option<std::time::Duration>,
}

#[derive(Clone, Default, Debug)]
pub struct Event {
    pub emitter: Address,
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn deserialize_address_from_array() {
        let bytes = [1u8; 20];
        let addr: Address = serde_cbor::from_slice(&serde_cbor::to_vec(&bytes).unwrap()).unwrap();
        assert_eq!(addr.0, bytes);
    }

    #[test]
    #[should_panic]
    fn fail_deserialize_address_from_short_array() {
        let bytes = [1u8; 19];
        serde_cbor::from_slice::<Address>(&serde_cbor::to_vec(&bytes).unwrap()).unwrap();
    }

    #[test]
    #[should_panic]
    fn fail_deserialize_address_from_long_array() {
        let bytes = [1u8; 21];
        serde_cbor::from_slice::<Address>(&serde_cbor::to_vec(&bytes).unwrap()).unwrap();
    }

    #[test]
    fn deserialize_address_from_slice() {
        let bytes = vec![1u8; 20];
        let addr: Address = serde_cbor::from_slice(&serde_cbor::to_vec(&bytes).unwrap()).unwrap();
        assert_eq!(&addr.0, bytes.as_slice());
    }

    #[test]
    #[should_panic]
    fn fail_deserialize_address_from_short_slice() {
        let bytes = vec![1u8; 19];
        serde_cbor::from_slice::<Address>(&serde_cbor::to_vec(&bytes).unwrap()).unwrap();
    }

    #[test]
    #[should_panic]
    fn fail_deserialize_address_from_long_slice() {
        let bytes = vec![1u8; 21];
        serde_cbor::from_slice::<Address>(&serde_cbor::to_vec(&bytes).unwrap()).unwrap();
    }
}
