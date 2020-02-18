use rustc_span::{symbol::Symbol, Span};
use syntax::{ast, mut_visit::MutVisitor as _, ptr::P};

use crate::{error::RpcError, visitor::syntax::Deborrower};

pub struct ParsedRpc {
    pub name: Symbol,
    pub kind: ParsedRpcKind,
    pub span: Span,
    pub output: ReturnType,
    sig: ast::FnSig,
}

impl ParsedRpc {
    pub fn try_new_maybe(
        service_ty: &P<ast::Ty>,
        impl_item: &ast::AssocItem,
    ) -> Option<Result<Self, Vec<RpcError>>> {
        let mut errors = Vec::new();

        let is_ctor = impl_item.ident.name == Symbol::intern("new");

        match impl_item.vis.node {
            ast::VisibilityKind::Public => (),
            _ if is_ctor => (),
            _ => return None,
        }

        let msig = match &impl_item.kind {
            ast::AssocItemKind::Fn(msig, _) => msig,
            _ => return None,
        };
        if impl_item
            .generics
            .params
            .iter()
            .fold(false, |has_generic, param| match param.kind {
                ast::GenericParamKind::Type { .. } => true,
                _ => has_generic,
            })
        {
            errors.push(RpcError::HasGenerics(impl_item.generics.span));
        }

        if let ast::Async::Yes { span, .. } = msig.header.asyncness {
            errors.push(RpcError::HasAsync(span));
        }

        if let ast::Unsafe::Yes(_) = msig.header.unsafety {
            errors.push(RpcError::Unsafe(impl_item.span));
        }

        match msig.header.ext {
            ast::Extern::None | ast::Extern::Implicit => (),
            _ => {
                // start from the `pub` to the fn ident
                // then slice from after the `pub ` to before the ` fn `
                let err_span = impl_item.span.until(impl_item.ident.span);
                let err_span = err_span.from_inner(rustc_span::InnerSpan::new(
                    4,
                    (err_span.hi().0 - err_span.lo().0) as usize - 4,
                ));
                errors.push(RpcError::HasAbi(err_span));
            }
        }

        let default_span = impl_item.attrs.iter().find_map(|attr| {
            let attr_path = match &attr.kind {
                ast::AttrKind::Normal(item) => &item.path,
                _ => return None,
            };
            if crate::utils::path_ends_with(&attr_path, &["oasis_std", "default"]) {
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
                match arg.pat.kind {
                    ast::PatKind::Ident(..) => (),
                    _ => errors.push(RpcError::BadArgPat(arg.pat.span)),
                }
            }
        }

        let ret_ty = ReturnType::new(&msig.decl.output);

        if is_ctor {
            let mut ret_ty_is_self = false;
            if let Some(ast::Ty {
                kind: ast::TyKind::Path(_, path),
                ..
            }) = &ret_ty.ty.as_ref().map(|p| &**p)
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
                if *mutability == ast::Mutability::Mut =>
            {
                true
            }
            _ => false,
        }
    }

    pub fn arg_names(&self) -> impl Iterator<Item = Symbol> + '_ {
        self.inputs().map(|arg| match arg.pat.kind {
            ast::PatKind::Ident(_, ident, _) => ident.name,
            _ => unreachable!("Checked during visitation."),
        })
    }

    pub fn arg_types<'a>(&'a self) -> impl Iterator<Item = P<ast::Ty>> + 'a {
        self.inputs().map(move |inp| {
            let mut ty = inp.ty.clone();
            Deborrower::default().visit_ty(&mut ty);
            ty
        })
    }

    fn inputs(&self) -> impl Iterator<Item = &ast::Param> {
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
    ty: Option<P<ast::Ty>>,
}

impl ReturnType {
    /// Returns a new `ReturnType` or `None` if error.
    fn new(ty: &ast::FunctionRetTy) -> Self {
        let mut ret_ty = Self {
            is_result: false,
            ty: None,
        };
        if let ast::FunctionRetTy::Ty(ty) = ty {
            match &ty.kind {
                ast::TyKind::Path(_, path) => {
                    let maybe_result = path.segments.last().unwrap();
                    ret_ty.is_result = maybe_result.ident.name == Symbol::intern("Result");
                    if !ret_ty.is_result {
                        if !ty.kind.is_unit() {
                            ret_ty.ty = Some(ty.clone());
                        }
                        return ret_ty;
                    }
                    let result = maybe_result;
                    if result.args.is_none() {
                        // Weird. It's a user-defined type named Result.
                        return Self {
                            is_result: false,
                            ty: Some(ty.clone()),
                        };
                    }
                    if let ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs {
                        args, ..
                    }) = &**result.args.as_ref().unwrap()
                    {
                        if let ast::GenericArg::Type(p_ty) = &args[0] {
                            ret_ty.ty = Some(p_ty.clone())
                        }
                    }
                }
                _ => ret_ty.ty = Some(ty.clone()),
            }
        };
        ret_ty
    }

    pub fn owned_ty(&self) -> Option<P<ast::Ty>> {
        self.ty.as_ref().map(|ty| {
            let mut ty = ty.clone();
            Deborrower::default().visit_ty(&mut ty);
            ty
        })
    }

    pub fn is_result(&self) -> bool {
        self.is_result
    }
}
