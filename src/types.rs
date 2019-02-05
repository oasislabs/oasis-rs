///! Provides primitive fixed size hash types.
///! Based on https://github.com/paritytech/pwasm-std/blob/61db712/src/types.rs

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
