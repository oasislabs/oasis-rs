use rustc::util::nodemap::FxHashMap;
use syntax::{
    ast,
    mut_visit::{self, MutVisitor as _},
    ptr::P,
    source_map::Span,
    visit,
};
use syntax_pos::symbol::{Ident, Symbol};

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
        if !crate::utils::path_ends_with(&mac.path, &["oasis_std", "service"]) {
            return;
        }
        if mac.tts.len() != 1 {
            return;
        }
        if let Some(ident) = mac
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
        if crate::utils::path_ends_with(&mac.path, &["std", "println"])
            || crate::utils::path_ends_with(&mac.path, &["std", "print"])
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

pub struct ArgLifetimeTransducer {
    next_lifetimes: Box<dyn Iterator<Item = ast::Lifetime>>,
}

impl Default for ArgLifetimeTransducer {
    fn default() -> Self {
        Self {
            next_lifetimes: box (b'a'..=b'z').map(|ch| {
                let ident = Symbol::intern(&format!("'{}", ch as char));
                ast::Lifetime {
                    id: ast::DUMMY_NODE_ID,
                    ident: Ident::with_dummy_span(ident),
                }
            }),
        }
    }
}

impl ArgLifetimeTransducer {
    pub fn transduce(&mut self, ty: &P<ast::Ty>) -> (Vec<Symbol>, P<ast::Ty>) {
        let mut vis = LifetimeInserter {
            next_lifetimes: &mut self.next_lifetimes,
            lifetimes: Vec::new(),
        };
        let mut ty = ty.clone();
        vis.visit_ty(&mut ty);
        (vis.lifetimes, ty)
    }
}

struct LifetimeInserter<'a> {
    next_lifetimes: &'a mut dyn Iterator<Item = ast::Lifetime>,
    lifetimes: Vec<Symbol>,
}

impl<'a> mut_visit::MutVisitor for LifetimeInserter<'a> {
    fn visit_ty(&mut self, ty: &mut P<ast::Ty>) {
        match &mut ty.node {
            ast::TyKind::Rptr(ref mut maybe_lifetime, _) => {
                if maybe_lifetime.is_none() {
                    *maybe_lifetime = Some(self.next_lifetimes.next().unwrap());
                }
                self.lifetimes
                    .push(maybe_lifetime.as_ref().unwrap().ident.name);
            }
            ast::TyKind::Path(_, path) => {
                if let Some(args) = &path.segments.last().unwrap().args {
                    if let ast::GenericArgs::AngleBracketed(ab_args) = &**args {
                        self.lifetimes
                            .extend(ab_args.args.iter().filter_map(|arg| match arg {
                                ast::GenericArg::Lifetime(lt) => Some(lt.ident.name),
                                _ => None,
                            }))
                    }
                }
            }
            _ => (),
        }
        mut_visit::noop_visit_ty(ty, self);
    }
}
