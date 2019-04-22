use super::serialize;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

construct_uint! {
    /// A 256-bits (4 64-bit word) fixed-size bigint type.
    pub struct U256(4);
}

construct_fixed_hash! {
    /// A 160 bits (20 bytes) hash type (aka `Address`).
    pub struct H160(20);
}

construct_fixed_hash! {
    /// A 256-bits (32 bytes) hash type.
    pub struct H256(32);
}

// Auto-impl `From` conversions between `H256` and `H160`.
impl_fixed_hash_conversions!(H256, H160);

macro_rules! impl_serde_hash {
    ($name: ident, $len: expr) => {
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut slice = [0u8; 2 + 2 * $len];
                serialize::serialize(&mut slice, &self.0, serializer)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let mut bytes = [0u8; $len];
                serialize::deserialize_check_len(
                    deserializer,
                    serialize::ExpectedLen::Exact(&mut bytes),
                )?;
                Ok($name(bytes))
            }
        }
    };
}

macro_rules! impl_serde_uint {
    ($name: ident, $len: expr) => {
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut slice = [0u8; 2 + 2 * $len * 8];
                let mut bytes = [0u8; $len * 8];
                self.to_big_endian(&mut bytes);
                serialize::serialize_uint(&mut slice, &bytes, serializer)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let mut bytes = [0u8; $len * 8];
                let wrote = serialize::deserialize_check_len(
                    deserializer,
                    serialize::ExpectedLen::Between(0, &mut bytes),
                )?;
                Ok(bytes[0..wrote].into())
            }
        }
    };
}

impl_serde_hash!(H160, 20);
impl_serde_hash!(H256, 32);
impl_serde_uint!(U256, 32);

/// Represents an address in ethereum context.
pub type Address = H160;

impl Address {
    pub fn transfer<'a, V: Into<&'a U256>>(
        &self,
        value: V,
    ) -> Result<(), crate::errors::ExtCallError> {
        crate::ext::transfer(self, value.into())
    }

    pub fn balance(&self) -> U256 {
        crate::ext::balance(self)
    }

    /// Creates an `Address` from a big-endian byte array.
    pub fn from_raw(bytes: *const u8) -> Self {
        Address::from_slice(unsafe { std::slice::from_raw_parts(bytes, 20) })
    }
}

impl H256 {
    pub fn from_raw(bytes: *const u8) -> Self {
        Self::from_slice(unsafe { std::slice::from_raw_parts(bytes, 32) })
    }
}

impl U256 {
    pub fn from_raw(bytes: *const u8) -> Self {
        Self::from_big_endian(unsafe { std::slice::from_raw_parts(bytes, 32) })
    }
}

impl From<U256> for H256 {
    fn from(uint: U256) -> H256 {
        let mut hash = H256::zero();
        uint.to_big_endian(hash.as_bytes_mut());
        hash
    }
}

impl<'a> From<&'a U256> for H256 {
    fn from(uint: &'a U256) -> H256 {
        let mut hash: H256 = H256::zero();
        uint.to_big_endian(hash.as_bytes_mut());
        hash
    }
}

impl From<H256> for U256 {
    fn from(hash: H256) -> U256 {
        U256::from(hash.as_bytes())
    }
}

impl<'a> From<&'a H256> for U256 {
    fn from(hash: &'a H256) -> U256 {
        U256::from(hash.as_bytes())
    }
}

impl U256 {
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr() as *const u8
    }
}

macro_rules! impl_partial_eq_for_uint {
    ($( $prim:ty ),+) => {
        $(
            impl PartialEq<$prim> for U256 {
                fn eq(&self, prim: &$prim) -> bool {
                    self.as_u64() == u64::from(*prim)
                }
            }
        )+
    };
}

impl_partial_eq_for_uint!(u8, u16, u32, u64);

macro_rules! impl_hash_from_prim {
    ($( $prim:ty ),+) => {
        $(
            impl From<$prim> for H256 {
                fn from(prim: $prim) -> Self {
                    let mut hash = Self::zero();
                    let prim_bytes = prim.to_be_bytes();
                    let nb = hash.0.len();
                    hash.0.as_mut()[(nb - prim_bytes.len())..].copy_from_slice(&prim_bytes);
                    hash
                }
            }

            impl From<$prim> for Address {
                fn from(prim: $prim) -> Self {
                    let mut hash = Self::zero();
                    let prim_bytes = prim.to_be_bytes();
                    let nb = hash.0.len();
                    hash.0.as_mut()[(nb - prim_bytes.len())..].copy_from_slice(&prim_bytes);
                    hash
                }
            }
        )+
    };
}

impl_hash_from_prim!(i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize);

#[cfg(test)]
mod tests {

    use super::{H160, H256, U256};
    use serde_json as ser;

    #[test]
    fn test_serialize_h160() {
        let tests = vec![
            (H160::from(0), "0x0000000000000000000000000000000000000000"),
            (H160::from(2), "0x0000000000000000000000000000000000000002"),
            (H160::from(15), "0x000000000000000000000000000000000000000f"),
            (H160::from(16), "0x0000000000000000000000000000000000000010"),
            (
                H160::from(1_000),
                "0x00000000000000000000000000000000000003e8",
            ),
            (
                H160::from(100_000),
                "0x00000000000000000000000000000000000186a0",
            ),
            (
                H160::from(u64::max_value()),
                "0x000000000000000000000000ffffffffffffffff",
            ),
        ];

        for (number, expected) in tests {
            assert_eq!(
                format!("{:?}", expected),
                ser::to_string_pretty(&number).unwrap()
            );
            assert_eq!(number, ser::from_str(&format!("{:?}", expected)).unwrap());
        }
    }

    #[test]
    fn test_serialize_h256() {
        let tests = vec![
            (
                H256::from(0),
                "0x0000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                H256::from(2),
                "0x0000000000000000000000000000000000000000000000000000000000000002",
            ),
            (
                H256::from(15),
                "0x000000000000000000000000000000000000000000000000000000000000000f",
            ),
            (
                H256::from(16),
                "0x0000000000000000000000000000000000000000000000000000000000000010",
            ),
            (
                H256::from(1_000),
                "0x00000000000000000000000000000000000000000000000000000000000003e8",
            ),
            (
                H256::from(100_000),
                "0x00000000000000000000000000000000000000000000000000000000000186a0",
            ),
            (
                H256::from(u64::max_value()),
                "0x000000000000000000000000000000000000000000000000ffffffffffffffff",
            ),
        ];

        for (number, expected) in tests {
            assert_eq!(
                format!("{:?}", expected),
                ser::to_string_pretty(&number).unwrap()
            );
            assert_eq!(number, ser::from_str(&format!("{:?}", expected)).unwrap());
        }
    }

    #[test]
    fn test_serialize_invalid() {
        assert!(ser::from_str::<H256>(
            "\"0x000000000000000000000000000000000000000000000000000000000000000\""
        )
        .unwrap_err()
        .is_data());
        assert!(ser::from_str::<H256>(
            "\"0x000000000000000000000000000000000000000000000000000000000000000g\""
        )
        .unwrap_err()
        .is_data());
        assert!(ser::from_str::<H256>(
            "\"0x00000000000000000000000000000000000000000000000000000000000000000\""
        )
        .unwrap_err()
        .is_data());
        assert!(ser::from_str::<H256>("\"\"").unwrap_err().is_data());
        assert!(ser::from_str::<H256>("\"0\"").unwrap_err().is_data());
        assert!(ser::from_str::<H256>("\"10\"").unwrap_err().is_data());
    }
}
