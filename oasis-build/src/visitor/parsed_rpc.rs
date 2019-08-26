use syntax::{
    ast, mut_visit::MutVisitor as _, print::pprust, ptr::P, source_map::Span, visit::Visitor as _,
};
use syntax_pos::symbol::Symbol;

use crate::error::RpcError;

pub struct ParsedRpc {
    pub name: Symbol,
    pub kind: ParsedRpcKind,
    pub span: Span,
    pub output: ReturnType,
    sig: ast::MethodSig,
}

impl ParsedRpc {
    pub fn try_new_maybe(
        service_ty: &P<ast::Ty>,
        impl_item: &ast::ImplItem,
    ) -> Option<Result<Self, Vec<RpcError>>> {
        let mut errors = Vec::new();

        let is_ctor = impl_item.ident.name == Symbol::intern("new");

        match impl_item.vis.node {
            ast::VisibilityKind::Public => (),
            _ if is_ctor => (),
            _ => return None,
        }

        let msig = match &impl_item.node {
            ast::ImplItemKind::Method(msig, _) => msig,
            _ => return None,
        };
        if !impl_item.generics.params.is_empty() {
            errors.push(RpcError::HasGenerics(impl_item.generics.span));
        }

        if let ast::IsAsync::Async { .. } = msig.header.asyncness.node {
            errors.push(RpcError::HasAsync(msig.header.asyncness.span));
        }

        if let ast::Unsafety::Unsafe = msig.header.unsafety {
            errors.push(RpcError::Unsafe(impl_item.span));
        }

        match msig.header.abi {
            rustc_target::spec::abi::Abi::Rust => (),
            _ => {
                // start from the `pub` to the fn ident
                // then slice from after the `pub ` to before the ` fn `
                let err_span = impl_item.span.until(impl_item.ident.span);
                let err_span = err_span.from_inner(syntax_pos::InnerSpan::new(
                    4,
                    (err_span.hi().0 - err_span.lo().0) as usize - 4,
                ));
                errors.push(RpcError::HasAbi(err_span));
            }
        }

        let default_span = impl_item.attrs.iter().find_map(|attr| {
            if crate::utils::path_ends_with(&attr.path, &["oasis_std", "default"]) {
                Some(attr.span)
            } else {
                None
            }
        });

        let mut args = msig.decl.inputs.iter();

        if !is_ctor {
            match args.next() {
                Some(arg) if !crate::utils::is_self_ref(&arg.ty) => {
                    errors.push(RpcError::MissingSelf(arg.pat.span.to(arg.pat.span)))
                }
                None => errors.push(RpcError::MissingSelf(impl_item.ident.span)),
                _ => (),
            }
        }
        match args.next() {
            Some(arg) if !crate::utils::is_context_ref(&arg.ty) => {
                errors.push(RpcError::MissingContext {
                    from_ctor: is_ctor,
                    span: arg.ty.span.to(arg.pat.span),
                })
            }
            None => errors.push(RpcError::MissingContext {
                from_ctor: is_ctor,
                span: impl_item.ident.span,
            }),
            _ => (),
        }

        if let Some(default_span) = default_span {
            if is_ctor {
                errors.push(RpcError::CtorIsDefault(default_span));
            }
            if let Some(arg) = args.next() {
                errors.push(RpcError::DefaultFnHasArg(arg.pat.span.to(arg.ty.span)));
            }
        } else {
            for arg in args {
                match arg.pat.node {
                    ast::PatKind::Ident(..) => (),
                    _ => errors.push(RpcError::BadArgPat(arg.pat.span)),
                }

                let mut ref_checker = super::syntax::RefChecker::default();
                ref_checker.visit_ty(&*arg.ty);
                if ref_checker.has_ref {
                    let mut suggested_ty = arg.ty.clone();
                    super::syntax::Deborrower {}.visit_ty(&mut suggested_ty);
                    errors.push(RpcError::BadArgTy {
                        span: arg.ty.span,
                        suggestion: pprust::ty_to_string(&suggested_ty),
                    });
                }
            }
        }

        let ret_ty = ReturnType::new(&msig.decl.output);

        if is_ctor {
            let mut ret_ty_is_self = false;
            if let Some(ast::Ty {
                node: ast::TyKind::Path(_, path),
                ..
            }) = &ret_ty.ty
            {
                ret_ty_is_self = path.segments.len() == 1
                    && path.segments[0].ident.name == Symbol::intern("Self");
            }
            if !ret_ty_is_self {
                errors.push(RpcError::BadCtorReturn {
                    self_ty: service_ty.clone().into_inner(),
                    span: msig.decl.output.span(),
                });
            }
        }

        Some(if errors.is_empty() {
            Ok(ParsedRpc {
                name: impl_item.ident.name,
                sig: msig.clone(),
                kind: if is_ctor {
                    ParsedRpcKind::Ctor
                } else if let Some(default_span) = default_span {
                    ParsedRpcKind::Default(default_span)
                } else {
                    ParsedRpcKind::Normal
                },
                output: ret_ty,
                span: impl_item.ident.span,
            })
        } else {
            Err(errors)
        })
    }
    pub fn is_mut(&self) -> bool {
        if let ParsedRpcKind::Ctor = self.kind {
            return false;
        }
        match &self.sig.decl.get_self().unwrap().node {
            ast::SelfKind::Value(mutability)
            | ast::SelfKind::Region(_, mutability)
            | ast::SelfKind::Explicit(_, mutability)
                if *mutability == ast::Mutability::Mutable =>
            {
                true
            }
            _ => false,
        }
    }

    pub fn arg_names(&self) -> impl Iterator<Item = Symbol> + '_ {
        self.inputs().map(|arg| match arg.pat.node {
            ast::PatKind::Ident(_, ident, _) => ident.name,
            _ => unreachable!("Checked during visitation."),
        })
    }

    pub fn arg_tys(&self) -> impl Iterator<Item = &ast::Ty> + '_ {
        self.inputs().map(|arg| &*arg.ty)
    }

    fn inputs(&self) -> impl Iterator<Item = &ast::Arg> {
        self.sig.decl.inputs.iter().skip(match self.kind {
            ParsedRpcKind::Ctor => 1, /* ctx */
            _ => 2,                   /* self, ctx */
        })
    }
}

#[derive(PartialEq, Eq)]
pub enum ParsedRpcKind {
    Ctor,
    Default(Span),
    Normal,
}

pub struct ReturnType {
    is_result: bool,
    ty: Option<ast::Ty>,
}

impl ReturnType {
    /// Returns a new `ReturnType` or `None` if error.
    fn new(ty: &ast::FunctionRetTy) -> Self {
        let mut ret_ty = Self {
            is_result: false,
            ty: None,
        };
        if let ast::FunctionRetTy::Ty(ty) = ty {
            match &ty.node {
                ast::TyKind::Path(_, path) => {
                    let maybe_result = path.segments.last().unwrap();
                    ret_ty.is_result = maybe_result.ident.name == Symbol::intern("Result");
                    if !ret_ty.is_result {
                        if !ty.node.is_unit() {
                            ret_ty.ty = Some(ty.clone().into_inner());
                        }
                        return ret_ty;
                    }
                    let result = maybe_result;
                    if result.args.is_none() {
                        // Weird. It's a user-defined type named Result.
                        return Self {
                            is_result: false,
                            ty: Some(ty.clone().into_inner()),
                        };
                    }
                    if let ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs {
                        args, ..
                    }) = &**result.args.as_ref().unwrap()
                    {
                        if let ast::GenericArg::Type(p_ty) = &args[0] {
                            ret_ty.ty = Some(p_ty.clone().into_inner())
                        }
                    }
                }
                _ => ret_ty.ty = Some(ty.clone().into_inner()),
            }
        };
        ret_ty
    }

    pub fn is_result(&self) -> bool {
        self.is_result
    }
}
