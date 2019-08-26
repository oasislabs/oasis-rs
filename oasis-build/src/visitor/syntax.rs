use rustc::util::nodemap::FxHashMap;
use syntax::{ast, mut_visit, ptr::P, source_map::Span, visit};
use syntax_pos::symbol::Symbol;

use super::parsed_rpc::ParsedRpc;
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
        if mac_.tts.len() != 1 {
            return;
        }
        if let Some(ident) = mac_
            .tts
            .trees()
            .next_with_joint()
            .and_then(|(tree, _)| match tree {
                syntax::tokenstream::TokenTree::Token(tok) => Some(tok),
                _ => None,
            })
            .and_then(|tok| tok.ident())
            .map(|(ident, _)| ident)
        {
            self.services.push(Service {
                span: mac.span,
                name: ident.name,
            });
        }
    }
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
                    match ParsedRpc::try_new_maybe(&service_ty, impl_item) {
                        None => (),
                        Some(Ok(rpc)) => {
                            self.rpcs.push(rpc);

                            let mut println_finder = PrintlnFinder::default();
                            syntax::visit::walk_impl_item(&mut println_finder, &impl_item);
                            self.println_spans.extend(&println_finder.println_spans);
                        }
                        Some(Err(errs)) => self.errors.extend(errs),
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
    pub has_ref: bool,
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
                        *ty = crate::utils::gen_ty(ast::TyKind::Path(
                            None,
                            ast::Path::from_ident(ast::Ident::from_str("String")),
                        ))
                    }
                }
                ast::TyKind::Slice(slice_ty) => {
                    let mut path = ast::Path::from_ident(ast::Ident::from_str("Vec"));
                    path.segments[0].args = Some(P(ast::GenericArgs::AngleBracketed(
                        ast::AngleBracketedArgs {
                            span: syntax_pos::DUMMY_SP,
                            args: vec![ast::GenericArg::Type(slice_ty.clone())],
                            constraints: Vec::new(),
                        },
                    )));
                    *ty = crate::utils::gen_ty(ast::TyKind::Path(None, path));
                }
                _ => (),
            }
        }
        mut_visit::noop_visit_ty(ty, self);
    }
}
