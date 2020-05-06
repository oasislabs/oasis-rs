mod common;
mod dispatcher;
pub mod imports;

pub struct ServiceDefinition {
    pub name: rustc_span::symbol::Symbol,
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
