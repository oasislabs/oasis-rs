use rustc::{
    hir::{
        self,
        intravisit::{self, Visitor},
    },
    ty::{AdtDef, TyCtxt, TyS},
    util::nodemap::{FxHashMap, FxHashSet, HirIdSet},
};

/// Collects RPC functions defined on the `Contract`.
/// From a high level:
/// 1. Finds the type on which `Contract` is implemented (call it `TheContract`).
/// 2. Finds all `impl <TheContract>` items and collects their public methods.
#[derive(Default)]
pub struct RpcCollector<'tcx> {
    // The following `Option`s are set during the course of visitation.
    // The `oasis_std::service` macro ensures that the name exists and
    // `RpcCollector::contract_name` will panic otherwise.
    contract_name: Option<syntax_pos::symbol::Ident>,
    contract_def: Option<hir::def::Def>,
    impl_item_ids: FxHashMap<hir::def::Def, HirIdSet>, // ids of `impl`s from which to collect RPC fns
    rpcs: Vec<(syntax_pos::symbol::Ident, &'tcx hir::FnDecl)>, // the collected RPC fns
}

impl<'tcx> RpcCollector<'tcx> {
    pub fn rpcs(&self) -> &[(syntax_pos::symbol::Ident, &'tcx hir::FnDecl)] {
        self.rpcs.as_slice()
    }

    pub fn contract_name(&self) -> &syntax_pos::symbol::Ident {
        self.contract_name.as_ref().unwrap()
    }
}

impl<'tcx> Visitor<'tcx> for RpcCollector<'tcx> {
    fn visit_item(&mut self, item: &'tcx hir::Item) {
        match &item.node {
            hir::ItemKind::Impl(_, _, _, _, Some(trait_ref), ty, _)
                if trait_ref.path.segments[0].ident.name == "Contract" =>
            {
                if let hir::TyKind::Path(hir::QPath::Resolved(_, path)) = &ty.node {
                    self.contract_name = Some(path.segments.iter().last().unwrap().ident);
                    self.contract_def = Some(path.def);
                }
            }
            hir::ItemKind::Impl(_, _, _, _, None /* `trait_ref` */, ty, impl_item_refs) => {
                if let hir::TyKind::Path(hir::QPath::Resolved(_, path)) = &ty.node {
                    self.impl_item_ids.insert(
                        path.def,
                        impl_item_refs.iter().map(|iir| iir.id.hir_id).collect(),
                    );
                }
            }
            _ => (),
        }
        hir::intravisit::walk_item(self, item);
    }

    // Runs after `Item`s have been visited, so `self.contract_def` is populated, if it exists.
    fn visit_impl_item(&mut self, impl_item: &'tcx hir::ImplItem) {
        // Ensure that `ImplItem` is a fn.
        let fn_decl = match &impl_item.node {
            hir::ImplItemKind::Method(hir::MethodSig { decl, .. }, _) => decl,
            _ => return,
        };
        // Ensure that the fn is an RPC (public fn) for the Contract.
        if !self
            .contract_def
            .and_then(|def| self.impl_item_ids.get(&def))
            .map(|itm_ids| itm_ids.contains(&impl_item.hir_id))
            .unwrap_or(false)
            || !impl_item.vis.node.is_pub()
        {
            return;
        }

        self.rpcs.push((impl_item.ident, fn_decl));
    }

    fn nested_visit_map<'this>(&'this mut self) -> intravisit::NestedVisitorMap<'this, 'tcx> {
        intravisit::NestedVisitorMap::None
    }
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
