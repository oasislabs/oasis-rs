use failure::Fail;

pub type Result<T> = std::result::Result<T, failure::Error>;

#[derive(Fail, Debug)]
#[fail(display = "Call to Wasm import failed.")]
pub struct ExtCallError;

#[derive(Fail, Debug, Eq, PartialEq)]
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
