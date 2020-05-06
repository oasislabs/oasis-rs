use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

use rustc::{
    hir::map::Map as HirMap,
    ty::{subst::SubstsRef, AdtDef, TyCtxt, TyKind, TyS},
};
use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{
    self,
    def::{DefKind, Res},
    hir_id::HirIdSet,
    intravisit,
};
use rustc_span::{symbol::Symbol, Span};

/// Collects public functions defined in `impl #service_name`.
pub struct AnalyzedRpcCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    service_name: Symbol,
    rpc_impls: HirIdSet,
    rpcs: Vec<(
        Symbol,
        &'tcx rustc_hir::FnDecl<'tcx>,
        &'tcx rustc_hir::Body<'tcx>,
    )>, /* the collected RPC fns */
}

impl<'tcx> AnalyzedRpcCollector<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, service_name: Symbol) -> Self {
        Self {
            tcx,
            service_name,
            rpc_impls: HirIdSet::default(),
            rpcs: Vec::new(),
        }
    }

    pub fn rpcs(
        &self,
    ) -> &[(
        Symbol,
        &'tcx rustc_hir::FnDecl<'tcx>,
        &'tcx rustc_hir::Body<'tcx>,
    )] {
        self.rpcs.as_slice()
    }
}

impl<'tcx> rustc_hir::itemlikevisit::ItemLikeVisitor<'tcx> for AnalyzedRpcCollector<'tcx> {
    fn visit_item(&mut self, item: &'tcx rustc_hir::Item) {
        if let rustc_hir::ItemKind::Impl {
            of_trait: None,
            self_ty: ty,
            ..
        } = &item.kind
        {
            if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
                if path.segments.last().unwrap().ident.name == self.service_name {
                    self.rpc_impls.insert(item.hir_id);
                }
            }
        }
    }

    fn visit_impl_item(&mut self, impl_item: &'tcx rustc_hir::ImplItem<'tcx>) {
        if let rustc_hir::ImplItemKind::Method(rustc_hir::FnSig { decl, .. }, body_id) =
            &impl_item.kind
        {
            if impl_item.vis.node.is_pub()
                && self
                    .rpc_impls
                    .contains(&self.tcx.hir().get_parent_item(impl_item.hir_id))
            {
                let body = self.tcx.hir().body(*body_id);
                self.rpcs.push((impl_item.ident.name, &decl, body));
            }
        }
    }

    fn visit_trait_item(&mut self, _trait_item: &'tcx rustc_hir::TraitItem) {}
}

/// Visits an RPC method's types and collects structs, unions, enums, and type aliases
/// that are not in a standard library crate.
pub struct DefinedTypeCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    def_tys: FxHashMap<DefinedType<'tcx>, Vec<Span>>,
}

impl<'tcx> DefinedTypeCollector<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            def_tys: FxHashMap::default(),
        }
    }

    pub fn def_tys(self) -> impl Iterator<Item = (DefinedType<'tcx>, Vec<Span>)> {
        self.def_tys.into_iter()
    }

    // called by `<DefinedTypeCollector as intravisit::Visitor>::visit_ty`.
    fn visit_sty(&mut self, ty: &'tcx TyS, originating_span: Span) {
        if let TyKind::Adt(adt_def, substs) = ty.kind {
            substs
                .types()
                .for_each(|ty| self.visit_sty(ty, originating_span));

            let def_ty = DefinedType {
                adt_def,
                substs,
                is_event: false,
            };
            if crate::utils::is_std(self.tcx.crate_name(adt_def.did.krate))
                || self.def_tys.contains_key(&def_ty)
            {
                return;
            }
            self.def_tys
                .entry(def_ty)
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

impl<'tcx> rustc_hir::intravisit::Visitor<'tcx> for DefinedTypeCollector<'tcx> {
    type Map = HirMap<'tcx>;

    fn visit_ty(&mut self, ty: &'tcx rustc_hir::Ty) {
        if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
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

    fn nested_visit_map<'this>(
        &'this mut self,
    ) -> intravisit::NestedVisitorMap<'this, HirMap<'tcx>> {
        intravisit::NestedVisitorMap::None
    }
}

/// Visits method bodies to find the structs of emitted events.
/// Visit all methods because events can be emitted from any context (incl. library functions).
/// The only constraint is that any event must be emitted in the current crate.
pub struct EventCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    def_tys: FxHashMap<DefinedType<'tcx>, Vec<Span>>,
}

impl<'tcx> EventCollector<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            def_tys: FxHashMap::default(),
        }
    }

    pub fn def_tys(self) -> impl Iterator<Item = (DefinedType<'tcx>, Vec<Span>)> {
        self.def_tys.into_iter()
    }
}

// This visit could be made more robust to other traits/methods named Event/emit by actually
// checking whether the types implement `oasis_std::exe::Event`, but this should suffice for now.
impl<'tcx> rustc_hir::intravisit::Visitor<'tcx> for EventCollector<'tcx> {
    type Map = HirMap<'tcx>;

    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr) {
        let emit_arg = match &expr.kind {
            rustc_hir::ExprKind::MethodCall(path_seg, _span, args)
                if path_seg.ident.to_string() == "emit" =>
            {
                Some(&args[0])
            }
            rustc_hir::ExprKind::Call(func_expr, args) => match &func_expr.kind {
                rustc_hir::ExprKind::Path(rustc_hir::QPath::Resolved(_, path))
                    if path.to_string().ends_with("Event::emit") =>
                {
                    Some(&args[0])
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(emit_arg) = emit_arg {
            let did = emit_arg.hir_id.owner_def_id();
            let emit_arg_ty = self.tcx.typeck_tables_of(did).expr_ty(&emit_arg);
            macro_rules! insert_def_ty {
                ($adt_def:expr, $substs:expr) => {
                    self.def_tys
                        .entry(DefinedType {
                            adt_def: $adt_def,
                            substs: $substs,
                            is_event: true,
                        })
                        .or_default()
                        .push(emit_arg.span)
                };
            }
            match emit_arg_ty.ty_adt_def() {
                Some(adt_def) => insert_def_ty!(adt_def, self.tcx.empty_substs_for_def_id(did)),
                None => {
                    if let TyKind::Ref(
                        _,
                        TyS {
                            kind: TyKind::Adt(adt_def, substs),
                            ..
                        },
                        _,
                    ) = emit_arg_ty.kind
                    {
                        insert_def_ty!(adt_def, substs)
                    }
                }
            }
        }
        intravisit::walk_expr(self, expr);
    }

    fn nested_visit_map<'this>(
        &'this mut self,
    ) -> intravisit::NestedVisitorMap<'this, HirMap<'tcx>> {
        intravisit::NestedVisitorMap::OnlyBodies(&self.tcx.hir())
    }
}

pub struct DefinedType<'a> {
    pub adt_def: &'a AdtDef,
    pub substs: SubstsRef<'a>,
    pub is_event: bool,
}

impl<'a> PartialOrd for DefinedType<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for DefinedType<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.adt_def.cmp(other.adt_def)
    }
}

impl<'a> PartialEq for DefinedType<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.adt_def == other.adt_def
    }
}

impl<'a> Eq for DefinedType<'a> {}

impl<'a> Hash for DefinedType<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.adt_def.hash(state);
    }
}
