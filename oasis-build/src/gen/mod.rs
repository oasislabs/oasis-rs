mod common;
mod dispatcher;
mod imports;
mod self_client;

pub use imports::build as build_imports;

pub struct ServiceDefinition<'ast> {
    pub name: syntax_pos::symbol::Symbol,
    pub ctor: &'ast syntax::ast::MethodSig,
    pub rpcs: Vec<crate::visitor::syntax::ParsedRpc>,
}

pub fn insert_oasis_bindings(
    build_ctx: crate::BuildContext,
    krate: &mut syntax::ast::Crate,
    service_def: ServiceDefinition,
) {
    dispatcher::insert(&build_ctx, krate, &service_def);
    self_client::insert(&build_ctx, krate, &service_def);
}
