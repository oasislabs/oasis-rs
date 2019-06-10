use mantle_types::ExtStatusCode;

#[derive(Debug, Eq, PartialEq, failure::Fail)]
pub enum Error {
    #[fail(display = "Unknown error occured.")]
    Unknown,

    #[fail(display = "Not enough funds to pay for transaction.")]
    InsufficientFunds,

    #[fail(display = "Execution ran out of gas.")]
    OutOfGas,

    #[fail(display = "Invalid input provided to a transaction.")]
    InvalidInput,

    #[fail(display = "Transaction failed with status code {}.", code)]
    Execution { code: u32, payload: Vec<u8> },
}

impl From<ExtStatusCode> for Error {
    fn from(code: ExtStatusCode) -> Self {
        match code {
            ExtStatusCode::InsufficientFunds => Error::InsufficientFunds,
            ExtStatusCode::OutOfGas => Error::OutOfGas,
            code if code.0 < u8::max_value() as u32 => Error::Unknown,
            code => Error::Execution {
                code: code.0,
                payload: crate::ext::fetch_err(),
            },
        }
    }
}
