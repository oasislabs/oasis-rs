#[macro_use]
extern crate fixed_hash;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate uint;

construct_uint! {
    /// A 256-bits (4 64-bit word) fixed-size bigint type.
    #[derive(Serialize, Deserialize)]
    pub struct U256(4);
}

construct_fixed_hash! {
    /// A 160 bits (20 bytes) hash type (aka `Address`).
    #[derive(Serialize, Deserialize)]
    pub struct H160(20);
}

construct_fixed_hash! {
    /// A 256-bits (32 bytes) hash type.
    #[derive(Serialize, Deserialize)]
    pub struct H256(32);
}

// Auto-impl `From` conversions between `H256` and `H160`.
impl_fixed_hash_conversions!(H256, H160);

/// Represents an address in ethereum context.
pub type Address = H160;

impl Address {
    /// Creates an `Address` from a big-endian byte array.
    pub unsafe fn from_raw(bytes: *const u8) -> Self {
        Address::from_slice(std::slice::from_raw_parts(bytes, 20))
    }
}

impl H256 {
    pub unsafe fn from_raw(bytes: *const u8) -> Self {
        Self::from_slice(std::slice::from_raw_parts(bytes, 32))
    }
}

impl U256 {
    pub unsafe fn from_raw(bytes: *const u8) -> Self {
        Self::from_big_endian(std::slice::from_raw_parts(bytes, 32))
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
