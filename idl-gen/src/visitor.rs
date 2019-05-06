use rustc::{
    hir::{self, intravisit},
    ty::{self, AdtDef, TyCtxt, TyS},
    util::nodemap::{FxHashSet, HirIdSet},
};
use syntax_pos::symbol::Symbol;

#[derive(Default)]
pub struct SyntaxPass {
    service_name: Option<Symbol>, // set to `Some` once pass is complete
}

impl SyntaxPass {
    pub fn service_name(&self) -> Option<Symbol> {
        self.service_name
    }
}

/// Identify the service name based on the existence of a `#[derive(Service)]`.
/// This is simpler than looking for the post-expansion `impl Service for T`
/// ref: 582b47c3d02f8cdbcdb187d1d67007ab613a070d/idl-gen/src/visitor.rs#L38-L45
impl<'ast> syntax::visit::Visitor<'ast> for SyntaxPass {
    fn visit_item(&mut self, item: &'ast syntax::ast::Item) {
        for attr in item.attrs.iter() {
            let meta = attr.meta();
            let metas = match &meta {
                Some(syntax::ast::MetaItem {
                    path,
                    node: syntax::ast::MetaItemKind::List(metas),
                    ..
                }) if path == &"derive" => metas,
                _ => continue,
            };

            for nested_meta in metas.iter() {
                let ident = match nested_meta.ident() {
                    Some(ident) => ident.as_str(),
                    None => continue,
                };
                if ident == "Service" {
                    self.service_name = Some(item.ident.name);
                }
            }
        }
        syntax::visit::walk_item(self, item);
    }

    fn visit_mac(&mut self, _mac: &'ast syntax::ast::Mac) {
        // The default implementation panics. Macro exprs exist but we don't need to look at them.
    }
}

/// Collects public functions defined in `impl #service_name`.
pub struct RpcCollector<'a, 'gcx, 'tcx> {
    tcx: TyCtxt<'a, 'gcx, 'tcx>,
    service_name: Symbol,
    rpc_impls: HirIdSet,
    rpcs: Vec<(Symbol, &'tcx hir::FnDecl)>, // the collected RPC fns
}

impl<'a, 'gcx, 'tcx> RpcCollector<'a, 'gcx, 'tcx> {
    pub fn new(tcx: TyCtxt<'a, 'gcx, 'tcx>, service_name: Symbol) -> Self {
        Self {
            tcx,
            service_name,
            rpc_impls: HirIdSet::default(),
            rpcs: Vec::new(),
        }
    }

    pub fn rpcs(&self) -> &[(Symbol, &'tcx hir::FnDecl)] {
        self.rpcs.as_slice()
    }
}

impl<'a, 'gcx, 'tcx> hir::itemlikevisit::ItemLikeVisitor<'tcx> for RpcCollector<'a, 'gcx, 'tcx> {
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
        if let hir::ImplItemKind::Method(hir::MethodSig { decl, .. }, _) = &impl_item.node {
            if impl_item.vis.node.is_pub()
                && self
                    .rpc_impls
                    .contains(&self.tcx.hir().get_parent_item(impl_item.hir_id))
            {
                self.rpcs.push((impl_item.ident.name, &decl));
            }
        }
    }

    fn visit_trait_item(&mut self, _trait_item: &'tcx hir::TraitItem) {}
}

/// Visits an RPC method's types and collects structs, unions, enums, and type aliases
/// that are not in a standard library crate.
pub struct DefinedTypeCollector<'a, 'gcx, 'tcx> {
    tcx: TyCtxt<'a, 'gcx, 'tcx>,
    adt_defs: FxHashSet<&'tcx AdtDef>, // maintain a `Set` to handle recursive types
}

impl<'a, 'gcx, 'tcx> DefinedTypeCollector<'a, 'gcx, 'tcx> {
    pub fn new(tcx: TyCtxt<'a, 'gcx, 'tcx>) -> Self {
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

impl<'a, 'gcx, 'tcx> hir::intravisit::Visitor<'tcx> for DefinedTypeCollector<'a, 'gcx, 'tcx> {
    fn visit_ty(&mut self, ty: &'tcx hir::Ty) {
        if let hir::TyKind::Path(hir::QPath::Resolved(_, path)) = &ty.node {
            use hir::def::Def;
            match path.def {
                Def::Struct(id) | Def::Union(id) | Def::Enum(id) | Def::TyAlias(id) => {
                    self.visit_sty(self.tcx.type_of(id));
                }
                _ => (),
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
pub struct EventCollector<'a, 'gcx, 'tcx> {
    tcx: TyCtxt<'a, 'gcx, 'tcx>,
    adt_defs: FxHashSet<&'tcx AdtDef>,
}

impl<'a, 'gcx, 'tcx> EventCollector<'a, 'gcx, 'tcx> {
    pub fn new(tcx: TyCtxt<'a, 'gcx, 'tcx>) -> Self {
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
// checking whether the types implement `oasis_std::exe::Event`, but this should suffice for now.
impl<'a, 'gcx, 'tcx> hir::intravisit::Visitor<'tcx> for EventCollector<'a, 'gcx, 'tcx> {
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
