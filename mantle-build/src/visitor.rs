use rustc::{
    hir::{self, intravisit, Crate},
    ty::{self, AdtDef, TyCtxt, TyS},
    util::nodemap::{FxHashMap, FxHashSet, HirIdSet},
};
use syntax::source_map::Span;
use syntax_pos::symbol::Symbol;

use crate::error::RpcError;

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

impl<'ast> syntax::visit::Visitor<'ast> for ServiceDefFinder {
    fn visit_item(&mut self, item: &'ast syntax::ast::Item) {
        for attr in item.attrs.iter() {
            let meta = attr.meta();
            let metas = match &meta {
                Some(syntax::ast::MetaItem {
                    path,
                    node: syntax::ast::MetaItemKind::List(metas),
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
                if let syntax::ast::ItemKind::Struct(variant_data, _) = &item.node {
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
        syntax::visit::walk_item(self, item);
    }

    fn visit_mac(&mut self, mac: &'ast syntax::ast::Mac) {
        let mac_ = &mac.node;
        if !crate::utils::path_ends_with(&mac_.path, &["mantle", "service"]) {
            return;
        }
        // Why not parse the `TokenStream`, you ask? Because the `TokenStream`
        // refers to sourcemap info not held by the anonymous `ParseSess` used
        // for one-off parsing.
        let service_ident = parse!(format!("{}", mac_.tts) => parse_ident);
        self.services.push(Service {
            span: mac.span,
            name: service_ident.name,
        });
    }
}

pub struct ParsedRpcCollector {
    service_name: Symbol,
    rpcs: Vec<(Symbol, syntax::ast::MethodSig)>,
    errors: Vec<RpcError>,
    struct_span: Span,
}

impl ParsedRpcCollector {
    pub fn new(service_name: Symbol) -> Self {
        Self {
            service_name,
            rpcs: Vec::new(),
            errors: Vec::new(),
            struct_span: Default::default(),
        }
    }

    pub fn struct_span(&self) -> Span {
        self.struct_span
    }

    pub fn into_rpcs(self) -> Result<Vec<(Symbol, syntax::ast::MethodSig)>, Vec<RpcError>> {
        if self.errors.is_empty() {
            Ok(self.rpcs)
        } else {
            Err(self.errors)
        }
    }

    fn is_self_ref(ty: &syntax::ast::Ty) -> bool {
        match &ty.node {
            syntax::ast::TyKind::Rptr(_, mut_ty) => mut_ty.ty.node.is_implicit_self(),
            _ => false,
        }
    }
}

impl<'ast> syntax::visit::Visitor<'ast> for ParsedRpcCollector {
    fn visit_item(&mut self, item: &'ast syntax::ast::Item) {
        match &item.node {
            syntax::ast::ItemKind::Struct(_, generics) if item.ident.name == self.service_name => {
                if !generics.params.is_empty() {
                    self.errors.push(RpcError::HasGenerics(generics.span))
                }

                self.struct_span = item.span;
            }
            syntax::ast::ItemKind::Impl(_, _, _, _, None, service_ty, impl_items)
                if match &service_ty.node {
                    syntax::ast::TyKind::Path(_, p) => *p == self.service_name,
                    _ => false,
                } =>
            {
                for impl_item in impl_items {
                    let mut errors = Vec::new();

                    let is_ctor = impl_item.ident.name == Symbol::intern("new");

                    match impl_item.vis.node {
                        syntax::ast::VisibilityKind::Public => (),
                        _ if is_ctor => (),
                        _ => continue,
                    }

                    let msig = match &impl_item.node {
                        syntax::ast::ImplItemKind::Method(msig, _) => msig,
                        _ => continue,
                    };
                    if !impl_item.generics.params.is_empty() {
                        errors.push(RpcError::HasGenerics(impl_item.generics.span));
                    }

                    if let syntax::ast::IsAsync::Async { .. } = msig.header.asyncness.node {
                        errors.push(RpcError::HasAsync(msig.header.asyncness.span));
                    }

                    if let syntax::ast::Unsafety::Unsafe = msig.header.unsafety {
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
                            self.errors.push(RpcError::HasAbi(err_span));
                        }
                    }

                    let mut args = msig.decl.inputs.iter();

                    if !is_ctor {
                        match args.next() {
                            Some(arg) if !Self::is_self_ref(&arg.ty) => {
                                errors.push(RpcError::MissingSelf(arg.pat.span.to(arg.pat.span)))
                            }
                            None => errors.push(RpcError::MissingSelf(impl_item.ident.span)),
                            _ => (),
                        }
                    }
                    match args.next() {
                        Some(arg) if !crate::utils::is_context_ref(&arg.ty) => {
                            self.errors.push(RpcError::MissingContext {
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
                    for arg in args {
                        match arg.pat.node {
                            syntax::ast::PatKind::Ident(..) => (),
                            _ => errors.push(RpcError::BadArgPat(arg.pat.span)),
                        }

                        let mut ref_checker = RefChecker::default();
                        ref_checker.visit_ty(&*arg.ty);
                        if ref_checker.has_ref {
                            use syntax::mut_visit::MutVisitor as _;
                            let mut suggested_ty = arg.ty.clone();
                            Deborrower {}.visit_ty(&mut suggested_ty);
                            errors.push(RpcError::BadArgTy {
                                span: arg.ty.span,
                                suggestion: syntax::print::pprust::ty_to_string(&suggested_ty),
                            });
                        }
                    }

                    let ret_ty = crate::utils::unpack_syntax_ret(&msig.decl.output);

                    if is_ctor {
                        let mut ret_ty_is_self = false;
                        if let crate::utils::ReturnType::Known(syntax::ast::Ty {
                            node: syntax::ast::TyKind::Path(_, path),
                            ..
                        }) = ret_ty.ty
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

                    if errors.is_empty() {
                        self.rpcs.push((impl_item.ident.name, msig.clone()));
                    } else {
                        self.errors.append(&mut errors);
                    }
                }
            }
            _ if item.ident.name == self.service_name => {
                self.errors.push(RpcError::BadStruct(item.span));
            }
            _ => (),
        }
        syntax::visit::walk_item(self, item);
    }

    fn visit_mac(&mut self, _mac: &'ast syntax::ast::Mac) {
        // The default implementation panics. They exist pre-expansion, but we don't need
        // to look at them. Hopefully nobody generates `Event` structs in a macro.
    }
}

#[derive(Default)]
pub struct RefChecker {
    has_ref: bool,
}

impl<'ast> syntax::visit::Visitor<'ast> for RefChecker {
    fn visit_ty(&mut self, ty: &'ast syntax::ast::Ty) {
        if let syntax::ast::TyKind::Rptr(..) = &ty.node {
            self.has_ref = true;
        }
        syntax::visit::walk_ty(self, ty);
    }
}

/// Collects public functions defined in `impl #service_name`.
pub struct AnalyzedRpcCollector<'a, 'tcx> {
    krate: &'a Crate,
    tcx: TyCtxt<'tcx>,
    service_name: Symbol,
    rpc_impls: HirIdSet,
    rpcs: Vec<(Symbol, &'tcx hir::FnDecl, &'a hir::Body)>, // the collected RPC fns
}

impl<'a, 'tcx> AnalyzedRpcCollector<'a, 'tcx> {
    pub fn new(krate: &'a Crate, tcx: TyCtxt<'tcx>, service_name: Symbol) -> Self {
        Self {
            krate,
            tcx,
            service_name,
            rpc_impls: HirIdSet::default(),
            rpcs: Vec::new(),
        }
    }

    pub fn rpcs(&self) -> &[(Symbol, &'tcx hir::FnDecl, &'a hir::Body)] {
        self.rpcs.as_slice()
    }
}

impl<'a, 'tcx> hir::itemlikevisit::ItemLikeVisitor<'tcx> for AnalyzedRpcCollector<'a, 'tcx> {
    fn visit_item(&mut self, item: &'tcx hir::Item) {
        if let hir::ItemKind::Impl(_, _, _, _, None /* `trait_ref` */, ty, _) = &item.node {
            if let hir::TyKind::Path(hir::QPath::Resolved(_, path)) = &ty.node {
                if path.segments.last().unwrap().ident.name == self.service_name {
                    self.rpc_impls.insert(item.hir_id);
                }
            }
        }
    }

    fn visit_impl_item(&mut self, impl_item: &'tcx hir::ImplItem) {
        if let hir::ImplItemKind::Method(hir::MethodSig { decl, .. }, body_id) = &impl_item.node {
            if impl_item.vis.node.is_pub()
                && self
                    .rpc_impls
                    .contains(&self.tcx.hir().get_parent_item(impl_item.hir_id))
            {
                let body = self.krate.body(*body_id);
                self.rpcs.push((impl_item.ident.name, &decl, body));
            }
        }
    }

    fn visit_trait_item(&mut self, _trait_item: &'tcx hir::TraitItem) {}
}

/// Visits an RPC method's types and collects structs, unions, enums, and type aliases
/// that are not in a standard library crate.
pub struct DefinedTypeCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    adt_defs: FxHashSet<&'tcx AdtDef>, // maintain a `Set` to handle recursive types
}

impl<'tcx> DefinedTypeCollector<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            adt_defs: FxHashSet::default(),
        }
    }

    pub fn adt_defs(self) -> Vec<&'tcx AdtDef> {
        self.adt_defs.into_iter().collect()
    }

    // called by `<DefinedTypeCollector as intravisit::Visitor>::visit_ty`.
    fn visit_sty(&mut self, ty: &'tcx TyS) {
        if let rustc::ty::TyKind::Adt(ref adt_def, ..) = ty.sty {
            if crate::utils::is_std(self.tcx.crate_name(adt_def.did.krate))
                || self.adt_defs.contains(adt_def)
            {
                return;
            }
            self.adt_defs.insert(adt_def);
            if adt_def.did.is_local() {
                for field in adt_def.all_fields() {
                    for inner_ty in self.tcx.type_of(field.did).walk() {
                        self.visit_sty(inner_ty);
                    }
                }
            }
        }
    }
}

impl<'tcx> hir::intravisit::Visitor<'tcx> for DefinedTypeCollector<'tcx> {
    fn visit_ty(&mut self, ty: &'tcx hir::Ty) {
        if let hir::TyKind::Path(hir::QPath::Resolved(_, path)) = &ty.node {
            use hir::def::{DefKind, Res};
            if let Res::Def(kind, id) = path.res {
                match kind {
                    DefKind::Struct | DefKind::Union | DefKind::Enum | DefKind::TyAlias => {
                        self.visit_sty(self.tcx.type_of(id));
                    }
                    _ => (),
                }
            }
        }
        intravisit::walk_ty(self, ty);
    }

    fn nested_visit_map<'this>(&'this mut self) -> intravisit::NestedVisitorMap<'this, 'tcx> {
        intravisit::NestedVisitorMap::None
    }
}

/// Visits method bodies to find the structs of emitted events.
/// Visit all methods because events can be emitted from any context (incl. library functions).
/// The only constraint is that any event must be emitted in the current crate.
pub struct EventCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    adt_defs: FxHashSet<&'tcx AdtDef>,
}

impl<'tcx> EventCollector<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            adt_defs: FxHashSet::default(),
        }
    }

    pub fn adt_defs(self) -> Vec<&'tcx AdtDef> {
        self.adt_defs.into_iter().collect()
    }
}

// This visit could be made more robust to other traits/methods named Event/emit by actually
// checking whether the types implement `mantle::exe::Event`, but this should suffice for now.
impl<'tcx> hir::intravisit::Visitor<'tcx> for EventCollector<'tcx> {
    fn visit_expr(&mut self, expr: &'tcx hir::Expr) {
        let emit_arg = match &expr.node {
            hir::ExprKind::MethodCall(path_seg, _span, args)
                if path_seg.ident.to_string() == "emit" =>
            {
                Some(&args[0])
            }
            hir::ExprKind::Call(func_expr, args) => match &func_expr.node {
                hir::ExprKind::Path(hir::QPath::Resolved(_, path))
                    if path.to_string().ends_with("Event::emit") =>
                {
                    Some(&args[0])
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(emit_arg) = emit_arg {
            let emit_arg_ty = self
                .tcx
                .typeck_tables_of(emit_arg.hir_id.owner_def_id())
                .expr_ty(&emit_arg);
            if let ty::TyKind::Ref(
                _,
                TyS {
                    sty: ty::TyKind::Adt(adt_def, _),
                    ..
                },
                _,
            ) = emit_arg_ty.sty
            {
                self.adt_defs.insert(&adt_def);
            }
            if let Some(adt_def) = emit_arg_ty.ty_adt_def() {
                self.adt_defs.insert(&adt_def);
            }
        }
        intravisit::walk_expr(self, expr);
    }

    fn nested_visit_map<'this>(&'this mut self) -> intravisit::NestedVisitorMap<'this, 'tcx> {
        intravisit::NestedVisitorMap::OnlyBodies(self.tcx.hir())
    }
}

pub struct Deborrower;
impl syntax::mut_visit::MutVisitor for Deborrower {
    fn visit_ty(&mut self, ty: &mut syntax::ptr::P<syntax::ast::Ty>) {
        if let syntax::ast::TyKind::Rptr(_, syntax::ast::MutTy { ty: refd_ty, .. }) = &ty.node {
            match &refd_ty.node {
                syntax::ast::TyKind::Path(None, path) => {
                    if path.segments.last().unwrap().ident.name == Symbol::intern("str") {
                        *ty = parse!("String" => parse_ty);
                    }
                }
                syntax::ast::TyKind::Slice(slice_ty) => {
                    *ty = parse!(format!("Vec<{}>",
                            syntax::print::pprust::ty_to_string(&slice_ty)) => parse_ty);
                }
                _ => (),
            }
        }
        syntax::mut_visit::noop_visit_ty(ty, self);
    }
}
