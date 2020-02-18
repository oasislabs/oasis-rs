use rustc_ast_pretty::pprust;
use rustc_span::{MultiSpan, Span};

// NB: `failure` won't work on these errors because `Span` isn't `Send`.

pub struct UnsupportedTypeError {
    pub type_name: String,
    pub span: Span,
}

impl std::fmt::Display for UnsupportedTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "`{}` cannot be converted to an RPC type", self.type_name)
    }
}

pub enum RpcError {
    BadArgPat(Span),
    BadStruct(Span),
    BadCtorReturn {
        self_ty: syntax::ast::Ty,
        span: Span,
    },
    CtorIsDefault(Span),
    DefaultFnHasArg(Span),
    HasAbi(Span),
    HasAsync(Span),
    HasGenerics(Span),
    MissingContext {
        from_ctor: bool,
        span: Span,
    },
    MissingSelf(Span),
    Unsafe(Span),
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use RpcError::*;
        match self {
            BadArgPat(..) => write!(f, "Argument name must be a valid identifier."),
            BadStruct(..) => write!(f, "Service state definition must have named fields."),
            BadCtorReturn { self_ty, .. } => write!(
                f,
                "Service constructor must return `Self` (aka `{}`)",
                pprust::ty_to_string(self_ty)
            ),
            CtorIsDefault(..) => write!(f, "Service constructor cannot be the default function."),
            DefaultFnHasArg(..) => {
                write!(f, "Default function cannot take arguments after `Context`.")
            }
            HasAbi(..) => write!(f, "RPC method cannot declare an ABI."),
            HasAsync(..) => write!(f, "RPC method cannot be async."),
            HasGenerics(..) => write!(f, "RPC definition cannot have generic parameters."),
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
            Unsafe(..) => write!(f, "RPC method cannot be unsafe."),
        }
    }
}

impl RpcError {
    pub fn span(&self) -> Span {
        use RpcError::*;
        match self {
            BadArgPat(span)
            | BadStruct(span)
            | BadCtorReturn { span, .. }
            | CtorIsDefault(span)
            | DefaultFnHasArg(span)
            | HasAbi(span)
            | HasAsync(span)
            | HasGenerics(span)
            | MissingContext { span, .. }
            | MissingSelf(span)
            | Unsafe(span) => *span,
        }
    }
}

pub enum RpcWarning {
    Println(MultiSpan),
}

impl std::fmt::Display for RpcWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use RpcWarning::*;
        match self {
            Println(..) => write!(f, "`println!` writes to the service output channel. If you meant to log debugging information, use `eprintln!` or `dbg!`."),
        }
    }
}

impl RpcWarning {
    pub fn span(&self) -> MultiSpan {
        use RpcWarning::*;
        match self {
            Println(span) => span.clone(),
        }
    }
}
