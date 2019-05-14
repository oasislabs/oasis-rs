#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct Address(pub [u8; 20]);

impl<T: AsRef<[u8]>> From<T> for Address {
    fn from(sl: T) -> Self {
        let sl = sl.as_ref();
        let mut arr = [0u8; std::mem::size_of::<Self>()];
        arr.copy_from_slice(sl);
        Self(arr)
    }
}
