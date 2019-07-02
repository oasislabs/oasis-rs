#[derive(Debug, failure::Fail)]
pub enum ImporterError {
    #[fail(display = "Invalid URL: {}", _0)]
    InvalidUrl(#[fail(cause)] url::ParseError),

    #[fail(display = "No importer for scheme `{}`", _0)]
    NoImporter(String),

    #[fail(display = "{}", _0)]
    Fail(#[fail(cause)] failure::Error),

    #[fail(display = "Wasm module missing mantle-interface section")]
    MissingInterfaceSection,

    #[fail(display = "Could not locate `{}`", _0)]
    NoImport(String),
}

