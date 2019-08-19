mod common;
mod dispatcher;
mod imports;

pub use imports::build as build_imports;

pub struct ServiceDefinition {
    pub name: syntax_pos::symbol::Symbol,
    pub ctor: crate::visitor::parsed_rpc::ParsedRpc,
    pub rpcs: Vec<crate::visitor::parsed_rpc::ParsedRpc>,
}

pub fn insert_oasis_bindings(
    build_ctx: crate::BuildContext,
    krate: &mut syntax::ast::Crate,
    service_def: ServiceDefinition,
) {
    dispatcher::insert(&build_ctx, krate, &service_def);
}
