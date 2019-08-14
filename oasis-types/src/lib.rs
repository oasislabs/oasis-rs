#[macro_use]
extern crate serde;

/// A 160-bit little-endian hash address type.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize)]
#[repr(C)]
pub struct Address(pub [u8; 20]);

impl Address {
    /// Creates an `Address` from a little-endian byte array.
    pub unsafe fn from_raw(bytes: *const u8) -> Self {
        let mut addr = Self::default();
        addr.0
            .copy_from_slice(std::slice::from_raw_parts(bytes, 20));
        addr
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    pub const fn len() -> usize {
        std::mem::size_of::<Self>()
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl blockchain_traits::Address for Address {
    fn path_repr(&self) -> String {
        hex::encode(self)
    }
}

impl std::str::FromStr for Address {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes: Vec<u8> = hex::decode(s)?;
        if bytes.len() != Address::len() {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut addr = Self::default();
        addr.0.copy_from_slice(&bytes);
        Ok(addr)
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl<'de> serde::de::Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        use serde::de;

        const EXPECTATION: &str = "20 bytes";

        struct AddressVisitor;
        impl<'de> de::Visitor<'de> for AddressVisitor {
            type Value = Address;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(EXPECTATION)
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let mut arr = [0; Self::Value::len()];

                if let Some(len) = seq.size_hint() {
                    if len != arr.len() {
                        return Err(de::Error::invalid_length(len, &EXPECTATION));
                    }
                }

                let mut i = 0;
                loop {
                    match seq.next_element()? {
                        Some(el) if i < arr.len() => arr[i] = el,
                        None if i == arr.len() => break,
                        _ => return Err(de::Error::invalid_length(i, &EXPECTATION)),
                    }
                    i += 1;
                }

                Ok(Address(arr))
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let mut arr = [0; std::mem::size_of::<Self::Value>()];
                if value.len() == arr.len() {
                    arr.copy_from_slice(value);
                    Ok(Address(arr))
                } else {
                    Err(de::Error::invalid_length(value.len(), &EXPECTATION))
                }
            }
        }

        deserializer.deserialize_any(AddressVisitor)
    }
}

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

pub struct AccountMeta {
    pub balance: u64,
    pub expiry: Option<std::time::Duration>,
}

impl blockchain_traits::AccountMeta for AccountMeta {
    fn balance(&self) -> u64 {
        self.balance
    }
}

pub struct Event {
    pub emitter: Address,
    pub topics: Vec<Vec<u8>>,
    pub data: Vec<u8>,
}

impl blockchain_traits::Event for Event {
    type Address = Address;

    fn emitter(&self) -> &Self::Address {
        &self.emitter
    }

    fn topics(&self) -> Vec<&[u8]> {
        self.topics.iter().map(Vec::as_slice).collect()
    }

    fn data(&self) -> &[u8] {
        self.data.as_slice()
    }
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
