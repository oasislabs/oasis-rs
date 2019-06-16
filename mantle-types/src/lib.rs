#[macro_use]
extern crate serde;

/// A 160-bit little-endian hash address type.
#[derive(
    Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize,
)]
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
        20
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

    fn topics(&self) -> Vec<Vec<u8>> {
        self.topics.clone()
    }

    fn data(&self) -> Vec<u8> {
        self.data.clone()
    }
}
