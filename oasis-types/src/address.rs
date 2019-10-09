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

    pub fn path_repr(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(hex::encode(self))
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
    fn deserialize_address_from_bytes() {
        let orig_addr = Address::default();
        let addr: Address = serde_cbor::from_slice(
            &serde_cbor::to_vec(&serde_bytes::Bytes::new(&orig_addr.0)).unwrap(),
        )
        .unwrap();
        assert_eq!(addr, orig_addr);
    }

    #[test]
    fn deserialize_address_from_bytes_bad() {
        let addr: Result<Address, _> = serde_cbor::from_slice(
            &serde_cbor::to_vec(&serde_bytes::Bytes::new(&[0u8; 19])).unwrap(),
        );
        assert!(addr.is_err());

        let addr: Result<Address, _> = serde_cbor::from_slice(
            &serde_cbor::to_vec(&serde_bytes::Bytes::new(&[0u8; 21])).unwrap(),
        );
        assert!(addr.is_err());
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

    #[test]
    fn convert_str() {
        use std::str::FromStr;

        let addr = Address([
            96, 255, 103, 244, 45, 95, 214, 205, 158, 83, 176, 57, 114, 69, 94, 82, 182, 223, 75,
            28,
        ]);
        let addr_str = "60ff67f42d5fd6cd9e53b03972455e52b6df4b1c";
        assert_eq!(&addr.path_repr(), std::path::Path::new(addr_str));
        assert_eq!(&format!("{:x}", addr), addr_str);
        assert_eq!(format!("{}", addr), format!("0x{}", addr_str));
        assert_eq!(Address::from_str(addr_str).unwrap(), addr);
        assert!(Address::from_str(&addr_str[1..]).is_err());
        assert!(Address::from_str(&format!("{}ab", addr_str)).is_err());
        assert!(Address::from_str("zz").is_err());
    }

    #[test]
    fn convert_raw() {
        let addr = Address([
            96, 255, 103, 244, 45, 95, 214, 205, 158, 83, 176, 57, 114, 69, 94, 82, 182, 223, 75,
            28,
        ]);
        assert_eq!(unsafe { Address::from_raw(addr.as_ptr()) }, addr);
    }
}
