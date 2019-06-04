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

#[derive(Debug)]
pub enum RpcError {
    BadArg(Span),
    CtorVis(Span),
    HasAbi(Span),
    HasAsync(Span),
    HasGenerics(Span),
    MissingContext {
        from_ctor: bool,
        span: Span,
    },
    MissingSelf(Span),
    BadCtorReturn {
        self_ty: syntax::ast::Ty,
        span: Span,
    },
    MissingOutput(Span),
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use RpcError::*;
        match self {
            BadArg(..) => write!(f, "Argument name must be a valid identifier."),
            BadCtorReturn { self_ty, .. } => {
                let self_ty_str = format!("{:?}", self_ty);
                write!(
                    f,
                    "Service constructor must return `Self` (aka `{}`)",
                    &self_ty_str["type(".len()..(self_ty_str.len() - 1)]
                )
            }
            CtorVis(..) => write!(f, "Service constructor must have `pub` visibility."),
            HasAbi(..) => write!(f, "RPC method cannot declare an ABI."),
            HasAsync(..) => write!(f, "RPC method cannot be async."),
            HasGenerics(..) => write!(f, "RPC method cannot have generic parameters."),
            MissingContext { from_ctor, .. } => {
                if *from_ctor {
                    write!(
                        f,
                        "Service constructor must take `&Context` as its first argument."
                    )
                } else {
                    write!(f, "RPC method must take `&Context` as its second argument.")
                }
            }
            MissingSelf(..) => write!(
                f,
                "RPC method must take `&self` or `&mut self` as its first argument."
            ),
            MissingOutput(..) => write!(f, "RPC method must return `Result`."),
        }
    }
}

impl RpcError {
    pub fn span(&self) -> Span {
        use RpcError::*;
        match self {
            BadArg(span)
            | CtorVis(span)
            | HasAbi(span)
            | HasAsync(span)
            | HasGenerics(span)
            | MissingContext { span, .. }
            | MissingSelf(span)
            | BadCtorReturn { span, .. }
            | MissingOutput(span) => *span,
        }
    }
}
