use rustc::{
    hir::{
        self,
        intravisit::{self, Visitor},
    },
    ty::{AdtDef, TyCtxt, TyS},
    util::nodemap::{FxHashMap, FxHashSet},
};
use syntax_pos::symbol::Symbol;

#[derive(Default)]
pub struct SyntaxPass {
    service_name: Option<syntax::source_map::symbol::Symbol>, // set to `Some` once pass is complete
    event_indices: FxHashMap<Symbol, Vec<Symbol>>,            // event_name -> field_name
}

impl SyntaxPass {
    pub fn service_name(&self) -> Option<Symbol> {
        self.service_name
    }

    pub fn event_indices(&self) -> &FxHashMap<Symbol, Vec<Symbol>> {
        &self.event_indices
    }
}

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
                if ident == "Contract" {
                    self.service_name = Some(item.ident.name);
                } else if ident == "Event" {
                    if let syntax::ast::ItemKind::Struct(variant_data, _) = &item.node {
                        let indexed_fields = variant_data
                            .fields()
                            .iter()
                            .filter_map(|field| {
                                field
                                    .attrs
                                    .iter()
                                    .find(|attr| attr.path == "indexed")
                                    .and_then(|_| field.ident.map(|ident| ident.name))
                            })
                            .collect();
                        self.event_indices.insert(item.ident.name, indexed_fields);
                    }
                }
            }
        }
        syntax::visit::walk_item(self, item);
    }

    fn visit_mac(&mut self, _mac: &'ast syntax::ast::Mac) {
        // The default implementation panics. They exist pre-expansion, but we don't need
        // to look at them. Hopefully nobody generates `Event` structs in a macro.
    }
}

/// Collects public functions defined in `impl #service_name`.
pub struct RpcCollector<'tcx> {
    service_name: Symbol,
    rpcs: Vec<(Symbol, &'tcx hir::FnDecl)>, // the collected RPC fns
}

impl<'tcx> RpcCollector<'tcx> {
    pub fn new(service_name: Symbol) -> Self {
        Self {
            service_name,
            rpcs: Vec::new(),
        }
    }

    pub fn rpcs(&self) -> &[(Symbol, &'tcx hir::FnDecl)] {
        self.rpcs.as_slice()
    }
}

impl<'tcx> Visitor<'tcx> for RpcCollector<'tcx> {
    fn visit_impl_item(&mut self, impl_item: &'tcx hir::ImplItem) {
        // Ensure that `ImplItem` is a fn.
        let fn_decl = match &impl_item.node {
            hir::ImplItemKind::Method(hir::MethodSig { decl, .. }, _) => decl,
            _ => return,
        };
        // Ensure that the fn is an RPC (public fn) for the Contract.
        if impl_item.ident.name != self.service_name || !impl_item.vis.node.is_pub() {
            return;
        }

        self.rpcs.push((impl_item.ident.name, fn_decl));
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
