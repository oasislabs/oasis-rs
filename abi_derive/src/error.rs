use crate::json::JsonError;

/// The result type for this procedural macro.
pub type Result<T> = std::result::Result<T, Error>;

/// Represents errors that may be encountered in
/// invokations of this procedural macro.
#[derive(Debug)]
pub struct Error {
    /// The kind of this error.
    kind: ErrorKind,
}

/// Kinds of errors that may be encountered in invokations
/// of this procedural macro.
#[derive(Debug)]
pub enum ErrorKind {
    /// An error that occured upon a JSON operation.
    JsonError(JsonError),
    /// When there was an invalid number of arguments passed to `eth_abi`.
    InvalidNumberOfArguments {
        /// The number of found arguments.
        found: usize,
    },
    /// When there is a malformatted argument passed to `eth_abi`.
    MalformattedArgument {
        /// The index of the malformatted argument.
        index: usize,
    },
}

impl From<JsonError> for Error {
    fn from(json_err: JsonError) -> Self {
        Error::from_kind(ErrorKind::JsonError(json_err))
    }
}

impl Error {
    /// Create an error from the given kind.
    ///
    /// # Note
    ///
    /// Just a private convenience constructor.
    fn from_kind(kind: ErrorKind) -> Self {
        Error { kind }
    }

    /// Returns the error kind of `self`.
    fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Returns an error representing that an invalid number of
    /// arguments passed to `eth_abi` have been found.
    pub fn invalid_number_of_arguments(found: usize) -> Self {
        assert!(found != 1 && found != 2);
        Error::from_kind(ErrorKind::InvalidNumberOfArguments { found })
    }

    /// Returns an error representing a malformatted argument passed to
    /// `eth_abi` has been found at the given index.
    pub fn malformatted_argument(index: usize) -> Self {
        assert!(index <= 1);
        Error::from_kind(ErrorKind::MalformattedArgument { index })
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match self.kind() {
            ErrorKind::JsonError(err) => write!(f, "{}", err),
            ErrorKind::InvalidNumberOfArguments { found } => write!(
                f,
                "found {} arguments passed to eth_abi but expected 1 or 2",
                found
            ),
            ErrorKind::MalformattedArgument { index } => write!(
                f,
                "found non-identifier argument at index {} passed to eth_abi",
                index
            ),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match self.kind() {
            ErrorKind::JsonError(err) => err.description(),
			ErrorKind::InvalidNumberOfArguments{ .. } => {
				"encountered an invalid number of arguments passed to eth_abi: expected 1 or 2"
			},
			ErrorKind::MalformattedArgument{ .. } => {
				"encountered malformatted argument passed to eth_abi: expected identifier (e.g. `Foo`))"
			}
		}
    }
}
