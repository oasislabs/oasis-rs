use syntax_pos::Span;

type TypeStr = String;

#[derive(Debug)]
pub enum UnsupportedTypeError {
    NotReprC(TypeStr, Span),
    ComplexEnum(Span),
    Unimplemented(TypeStr, Span),
}

impl std::fmt::Display for UnsupportedTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use UnsupportedTypeError::*;
        match self {
            NotReprC(ty_str, ..) => write!(f, "`{}` cannot be converted to an RPC type", ty_str),
            ComplexEnum(..) => write!(f, "Tagged unions cannot (yet) be converted to an RPC type"),
            Unimplemented(ty_str, ..) => write!(f, "Unimplemented RPC type: `{}`", ty_str),
        }
    }
}

impl UnsupportedTypeError {
    pub fn span(&self) -> Span {
        use UnsupportedTypeError::*;
        match self {
            NotReprC(_, span) | ComplexEnum(span) | Unimplemented(_, span) => *span,
        }
    }
}
