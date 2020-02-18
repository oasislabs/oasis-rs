use std::fmt;

/// A 160-bit little-endian hash address type.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(C)]
pub struct Address(pub [u8; 20]);

impl Address {
    /// Creates an `Address` from a little-endian byte array.
    ///
    /// # Safety
    ///
    /// Requires that `bytes` reference 20 bytes of static memory.
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

    /// Alias for `Address::default()`.
    pub fn zero() -> Self {
        Self::default()
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

#[cfg(feature = "serde")]
const _IMPL_SERDE_FOR_ADDRESS: () = {
    impl oasis_borsh::BorshSerialize for Address {
        fn serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
            writer.write_all(&self.0)
        }
    }

    impl oasis_borsh::BorshDeserialize for Address {
        fn deserialize<R: std::io::Read>(reader: &mut R) -> Result<Self, std::io::Error> {
            let mut addr = Address::default();
            reader.read_exact(&mut addr.0)?;
            Ok(addr)
        }
    }
};

#[cfg(test)]
mod tests {
    use super::*;

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

#[cfg(all(test, feature = "serde"))]
mod serde_tests {
    use super::*;

    use oasis_borsh::{BorshDeserialize as _, BorshSerialize as _};

    #[test]
    fn roundtrip_serialize_address() {
        let bytes = [1u8; 20];
        let addr = Address::try_from_slice(&Address(bytes).try_to_vec().unwrap()).unwrap();
        assert_eq!(addr.0, bytes);
    }

    #[test]
    #[should_panic]
    fn fail_deserialize_address_short() {
        let bytes = [1u8; 19];
        Address::try_from_slice(&bytes.try_to_vec().unwrap()).unwrap();
    }

    #[test]
    #[should_panic]
    fn fail_deserialize_address_long() {
        let bytes = [1u8; 21];
        Address::try_from_slice(&bytes.as_ref().try_to_vec().unwrap()).unwrap();
    }
}
