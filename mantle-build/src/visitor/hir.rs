use rustc::{
    hir::{self, intravisit, Crate},
    ty::{self, AdtDef, TyCtxt, TyS},
    util::nodemap::{FxHashMap, HirIdSet},
};
use syntax::source_map::Span;
use syntax_pos::symbol::Symbol;

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
    adt_defs: FxHashMap<&'tcx AdtDef, Vec<Span>>,
}

impl<'tcx> DefinedTypeCollector<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            adt_defs: FxHashMap::default(),
        }
    }

    pub fn adt_defs(self) -> impl Iterator<Item = (&'tcx AdtDef, Vec<Span>)> {
        self.adt_defs.into_iter()
    }

    // called by `<DefinedTypeCollector as intravisit::Visitor>::visit_ty`.
    fn visit_sty(&mut self, ty: &'tcx TyS, originating_span: Span) {
        if let ty::TyKind::Adt(adt_def, ..) = ty.sty {
            if crate::utils::is_std(self.tcx.crate_name(adt_def.did.krate))
                || self.adt_defs.contains_key(adt_def)
            {
                return;
            }
            self.adt_defs
                .entry(adt_def)
                .or_default()
                .push(originating_span);
            if !adt_def.did.is_local() {
                return;
            }
            for field in adt_def.all_fields() {
                for inner_ty in self.tcx.type_of(field.did).walk() {
                    self.visit_sty(inner_ty, self.tcx.def_span(field.did));
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
                        self.visit_sty(self.tcx.type_of(id), ty.span);
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
    adt_defs: FxHashMap<&'tcx AdtDef, Vec<Span>>,
}

impl<'tcx> EventCollector<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            adt_defs: FxHashMap::default(),
        }
    }

    pub fn adt_defs(self) -> impl Iterator<Item = (&'tcx AdtDef, Vec<Span>)> {
        self.adt_defs.into_iter()
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
            emit_arg_ty
                .ty_adt_def()
                .or_else(|| match emit_arg_ty.sty {
                    ty::TyKind::Ref(
                        _,
                        TyS {
                            sty: ty::TyKind::Adt(adt_def, _),
                            ..
                        },
                        _,
                    ) => Some(adt_def),
                    _ => None,
                })
                .map(|adt_def| {
                    self.adt_defs
                        .entry(adt_def)
                        .or_default()
                        .push(emit_arg.span);
                });
        }
        intravisit::walk_expr(self, expr);
    }

    fn nested_visit_map<'this>(&'this mut self) -> intravisit::NestedVisitorMap<'this, 'tcx> {
        intravisit::NestedVisitorMap::OnlyBodies(self.tcx.hir())
    }
}
