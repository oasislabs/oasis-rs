pub use borsh::{BorshDeserialize as Deserialize, BorshSerialize as Serialize};

pub fn encode<T: Serialize>(obj: &T) -> Result<Vec<u8>, std::io::Error> {
    obj.try_to_vec()
}

pub fn decode<T: Deserialize>(bytes: &[u8]) -> Result<T, std::io::Error> {
    T::try_from_slice(bytes)
}
