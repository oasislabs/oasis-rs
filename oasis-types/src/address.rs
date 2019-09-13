use std::fmt;

/// A 160-bit little-endian hash address type.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize)]
#[repr(C)]
#[serde(transparent)]
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

    // Alias for `mem::size_of::<Address>()`.
    pub const fn size() -> usize {
        std::mem::size_of::<Self>()
    }

    pub fn path_repr(&self) -> String {
        hex::encode(self)
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl std::str::FromStr for Address {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes: Vec<u8> = hex::decode(s)?;
        if bytes.len() != Address::size() {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut addr = Self::default();
        addr.0.copy_from_slice(&bytes);
        Ok(addr)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl fmt::LowerHex for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
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

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(EXPECTATION)
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let mut addr = Address::default();

                if let Some(len) = seq.size_hint() {
                    if len != Address::size() {
                        return Err(de::Error::invalid_length(len, &EXPECTATION));
                    }
                }

                let mut i = 0;
                loop {
                    match seq.next_element()? {
                        Some(el) if i < Address::size() => addr.0[i] = el,
                        None if i == Address::size() => break,
                        _ => return Err(de::Error::invalid_length(i, &EXPECTATION)),
                    }
                    i += 1;
                }

                Ok(addr)
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let mut addr = Address::default();
                if value.len() == Address::size() {
                    addr.0.copy_from_slice(value);
                    Ok(addr)
                } else {
                    Err(de::Error::invalid_length(value.len(), &EXPECTATION))
                }
            }
        }

        deserializer.deserialize_any(AddressVisitor)
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
