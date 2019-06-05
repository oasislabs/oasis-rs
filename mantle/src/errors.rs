#[derive(failure::Fail, Debug)]
#[fail(display = "Call to Wasm import failed.")]
pub struct ExtCallError;

#[derive(failure::Fail, Debug, Eq, PartialEq)]
pub enum AbiError {
    #[fail(display = "Invalid bool for provided input")]
    InvalidBool,
    #[fail(display = "Invalid u32 for provided input")]
    InvalidU32,
    #[fail(display = "Invalid u64 for provided input")]
    InvalidU64,
    #[fail(display = "Unexpected end of stream")]
    UnexpectedEof,
    #[fail(display = "Invalid padding for fixed type")]
    InvalidPadding,
    #[fail(display = "Other error")]
    Other,
}
