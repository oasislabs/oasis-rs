#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Display,
    LowerHex,
    UpperHex,
    FromStr,
    From,
    Into,
    Add,
    Div,
    Mul,
    Rem,
    Sub,
    AddAssign,
    DivAssign,
    MulAssign,
    RemAssign,
    SubAssign,
)]
#[cfg_attr(
    feature = "serde",
    derive(oasis_borsh::BorshSerialize, oasis_borsh::BorshDeserialize)
)]
#[repr(C)]
pub struct Balance(pub u128);

impl Balance {
    // Alias for `mem::size_of::<Balance>()`.
    pub const fn size() -> usize {
        std::mem::size_of::<Self>()
    }
}

macro_rules! impl_interop_with_prims {
    ($($prim:ty),+) => {
        $(
            impl PartialEq<$prim> for Balance {
                fn eq(&self, prim: &$prim) -> bool {
                    use std::convert::TryFrom;
                    u128::try_from(*prim).map(|p| p == self.0).unwrap_or_default()
                }
            }

            impl PartialOrd<$prim> for Balance {
                fn partial_cmp(&self, prim: &$prim) -> Option<std::cmp::Ordering> {
                    use std::convert::TryFrom;
                    u128::try_from(*prim).ok().map(|p| self.0.cmp(&p))
                }
            }
        )+
    }
}

impl_interop_with_prims!(u8, i8, u16, i16, u32, i32, u64, i64, u128, i128);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let mut bal = Balance::from(3);
        assert_eq!(bal - Balance::from(2), Balance::from(1));
        bal += 1u128.into();
        assert_eq!(bal, 4)
    }

    #[test]
    fn test_mul() {
        let mut bal = Balance::from(3);
        bal *= 2;
        assert_eq!(u128::from(bal), 6u128);
        assert_eq!(bal % 4, Balance(2));
        assert_eq!(bal / 4, 1);
    }

    #[test]
    fn test_from_str() {
        use std::str::FromStr;
        assert!(Balance::from_str(&u128::max_value().to_string()).unwrap() == u128::max_value());
    }

    #[test]
    fn test_cmp() {
        assert!(Balance(1) < 2);
        assert!(Balance(1) == 1);
    }
}
