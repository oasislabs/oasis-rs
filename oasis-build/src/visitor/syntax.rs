use rustc::util::nodemap::FxHashMap;
use syntax::{
    ast,
    mut_visit::{self, MutVisitor},
    print::pprust,
    ptr::P,
    source_map::Span,
    visit::{self, Visitor},
};
use syntax_pos::symbol::Symbol;

use crate::error::{RpcError, RpcWarning};

#[derive(Default)]
pub struct ServiceDefFinder {
    services: Vec<Service>,
    event_indexed_fields: FxHashMap<Symbol, Vec<Symbol>>, // event_name -> field_name
}

#[derive(Debug)]
pub struct Service {
    pub span: Span,
    pub name: Symbol,
}

/// Identifies the main `Service` and `Event` definitions.
impl ServiceDefFinder {
    pub fn get(self) -> (Vec<Service>, FxHashMap<Symbol, Vec<Symbol>>) {
        (self.services, self.event_indexed_fields)
    }
}

impl<'ast> visit::Visitor<'ast> for ServiceDefFinder {
    fn visit_item(&mut self, item: &'ast ast::Item) {
        for attr in item.attrs.iter() {
            let meta = attr.meta();
            let metas = match &meta {
                Some(ast::MetaItem {
                    path,
                    node: ast::MetaItemKind::List(metas),
                    ..
                }) if *path == Symbol::intern("derive") => metas,
                _ => continue,
            };

            for nested_meta in metas.iter() {
                let ident = match nested_meta.ident() {
                    Some(ident) => ident.as_str(),
                    None => continue,
                };
                if ident != "Event" {
                    return;
                }
                if let ast::ItemKind::Struct(variant_data, _) = &item.node {
                    let indexed_fields = variant_data
                        .fields()
                        .iter()
                        .filter_map(|field| {
                            field
                                .attrs
                                .iter()
                                .find(|attr| attr.path == Symbol::intern("indexed"))
                                .and_then(|_| field.ident.map(|ident| ident.name))
                        })
                        .collect();
                    self.event_indexed_fields
                        .insert(item.ident.name, indexed_fields);
                }
            }
        }
        visit::walk_item(self, item);
    }

    fn visit_mac(&mut self, mac: &'ast ast::Mac) {
        let mac_ = &mac.node;
        if !crate::utils::path_ends_with(&mac_.path, &["oasis_std", "service"]) {
            return;
        }
        // Why not parse the `TokenStream`, you ask? Because the `TokenStream`
        // refers to sourcemap info not held by the anonymous `ParseSess` used
        // for one-off parsing.
        let service_ident = match try_parse!(format!("{}", mac_.tts) => parse_ident) {
            Ok(ident) => ident,
            Err(_) => return,
        };
        self.services.push(Service {
            span: mac.span,
            name: service_ident.name,
        });
    }
}

pub struct ParsedRpc {
    pub name: Symbol,
    pub sig: ast::MethodSig,
    pub kind: ParsedRpcKind,
    pub span: Span,
}

#[derive(PartialEq, Eq)]
pub enum ParsedRpcKind {
    Ctor,
    Default(Span),
    Normal,
}

pub struct ParsedRpcCollector {
    service_name: Symbol,
    rpcs: Vec<ParsedRpc>,
    errors: Vec<RpcError>,
    struct_span: Option<Span>,
    println_spans: Vec<Span>,
}

impl ParsedRpcCollector {
    pub fn new(service_name: Symbol) -> Self {
        Self {
            service_name,
            rpcs: Vec::new(),
            errors: Vec::new(),
            struct_span: None,
            println_spans: Vec::new(),
        }
    }

    pub fn struct_span(&self) -> Option<Span> {
        self.struct_span
    }

    pub fn into_rpcs(self) -> (Result<Vec<ParsedRpc>, Vec<RpcError>>, Vec<RpcWarning>) {
        let mut warnings = Vec::new();
        if !self.println_spans.is_empty() {
            warnings.push(RpcWarning::Println(self.println_spans.into()))
        }

        (
            if self.errors.is_empty() {
                Ok(self.rpcs)
            } else {
                Err(self.errors)
            },
            warnings,
        )
    }
}

impl<'ast> visit::Visitor<'ast> for ParsedRpcCollector {
    fn visit_item(&mut self, item: &'ast ast::Item) {
        match &item.node {
            ast::ItemKind::Struct(_, generics) if item.ident.name == self.service_name => {
                if !generics.params.is_empty() {
                    self.errors.push(RpcError::HasGenerics(generics.span))
                }

                self.struct_span = Some(item.span);
            }
            ast::ItemKind::Impl(_, _, _, _, None, service_ty, impl_items)
                if match &service_ty.node {
                    ast::TyKind::Path(_, p) => *p == self.service_name,
                    _ => false,
                } =>
            {
                for impl_item in impl_items {
                    match check_parsed_rpc(&service_ty, impl_item) {
                        Ok(Some(rpc)) => {
                            self.rpcs.push(rpc);

                            let mut println_finder = PrintlnFinder::default();
                            syntax::visit::walk_impl_item(&mut println_finder, &impl_item);
                            self.println_spans.extend(&println_finder.println_spans);
                        }
                        Ok(None) => (),
                        Err(errs) => self.errors.extend(errs),
                    }
                }
            }
            _ if item.ident.name == self.service_name => {
                self.errors.push(RpcError::BadStruct(item.span));
            }
            _ => (),
        }
        visit::walk_item(self, item);
    }

    fn visit_mac(&mut self, _mac: &'ast ast::Mac) {
        // The default implementation panics. They exist pre-expansion, but we don't need
        // to look at them. Hopefully nobody generates `Event` structs in a macro.
    }
}

fn check_parsed_rpc(
    service_ty: &P<ast::Ty>,
    impl_item: &ast::ImplItem,
) -> Result<Option<ParsedRpc>, Vec<RpcError>> {
    let mut errors = Vec::new();

    let is_ctor = impl_item.ident.name == Symbol::intern("new");

    match impl_item.vis.node {
        ast::VisibilityKind::Public => (),
        _ if is_ctor => (),
        _ => return Ok(None),
    }

    let msig = match &impl_item.node {
        ast::ImplItemKind::Method(msig, _) => msig,
        _ => return Ok(None),
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

            let mut ref_checker = RefChecker::default();
            ref_checker.visit_ty(&*arg.ty);
            if ref_checker.has_ref {
                let mut suggested_ty = arg.ty.clone();
                Deborrower {}.visit_ty(&mut suggested_ty);
                errors.push(RpcError::BadArgTy {
                    span: arg.ty.span,
                    suggestion: pprust::ty_to_string(&suggested_ty),
                });
            }
        }
    }

    let ret_ty = crate::utils::unpack_syntax_ret(&msig.decl.output);

    if is_ctor {
        let mut ret_ty_is_self = false;
        if let crate::utils::ReturnType::Known(ast::Ty {
            node: ast::TyKind::Path(_, path),
            ..
        }) = ret_ty.ty
        {
            ret_ty_is_self =
                path.segments.len() == 1 && path.segments[0].ident.name == Symbol::intern("Self");
        }
        if !ret_ty_is_self {
            errors.push(RpcError::BadCtorReturn {
                self_ty: service_ty.clone().into_inner(),
                span: msig.decl.output.span(),
            });
        }
    }

    if errors.is_empty() {
        Ok(Some(ParsedRpc {
            name: impl_item.ident.name,
            sig: msig.clone(),
            kind: if is_ctor {
                ParsedRpcKind::Ctor
            } else if let Some(default_span) = default_span {
                ParsedRpcKind::Default(default_span)
            } else {
                ParsedRpcKind::Normal
            },
            span: impl_item.ident.span,
        }))
    } else {
        Err(errors)
    }
}

#[derive(Default)]
struct PrintlnFinder {
    pub println_spans: Vec<Span>,
}

impl<'ast> visit::Visitor<'ast> for PrintlnFinder {
    fn visit_mac(&mut self, mac: &'ast ast::Mac) {
        if crate::utils::path_ends_with(&mac.node.path, &["std", "println"])
            || crate::utils::path_ends_with(&mac.node.path, &["std", "print"])
        {
            self.println_spans.push(mac.span);
        }
    }
}

#[derive(Default)]
pub struct RefChecker {
    has_ref: bool,
}

impl<'ast> visit::Visitor<'ast> for RefChecker {
    fn visit_ty(&mut self, ty: &'ast ast::Ty) {
        if let ast::TyKind::Rptr(..) = &ty.node {
            self.has_ref = true;
        }
        visit::walk_ty(self, ty);
    }
}

pub struct Deborrower;

impl mut_visit::MutVisitor for Deborrower {
    fn visit_ty(&mut self, ty: &mut P<ast::Ty>) {
        if let ast::TyKind::Rptr(_, ast::MutTy { ty: refd_ty, .. }) = &ty.node {
            match &refd_ty.node {
                ast::TyKind::Path(None, path) => {
                    if path.segments.last().unwrap().ident.name == Symbol::intern("str") {
                        *ty = parse!("String" => parse_ty);
                    }
                }
                ast::TyKind::Slice(slice_ty) => {
                    *ty = parse!(format!("Vec<{}>",
                            pprust::ty_to_string(&slice_ty)) => parse_ty);
                }
                _ => (),
            }
        }
        mut_visit::noop_visit_ty(ty, self);
    }
}
