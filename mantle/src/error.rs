#[derive(Debug, Eq, PartialEq, failure::Fail)]
pub enum Error {
    #[fail(display = "Unknown error occured.")]
    Unknown,

    #[fail(display = "Not enough funds to pay for transaction.")]
    InsufficientFunds,

    #[fail(display = "Invalid input provided to a transaction.")]
    InvalidInput,

    #[fail(display = "No account at destination of transaction.")]
    NoAccount,

    #[fail(display = "Transaction failed with status code {}.", code)]
    Execution { code: u32, payload: Vec<u8> },
}
