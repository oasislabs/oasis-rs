use std::{io::Write, path::Path};

use syntax::{
    ast::{Arg, Block, Crate, Item, ItemKind, MethodSig, StmtKind},
    print::pprust,
    ptr::P,
};
use syntax_pos::symbol::Symbol;

use crate::{
    visitor::syntax::{ParsedRpc, ParsedRpcKind},
    BuildContext, BuildTarget,
};

use super::ServiceDefinition;

pub fn insert(build_ctx: &BuildContext, krate: &mut Crate, service_def: &ServiceDefinition) {
    let BuildContext {
        out_dir,
        crate_name,
        ..
    } = build_ctx;

    let ServiceDefinition {
        name: service_name,
        rpcs,
        ctor,
    } = service_def;

    let client_impl = if build_ctx.target == BuildTarget::Wasi {
        build_wasi_client(service_def)
    } else {
        build_test_client(service_def)
    };

    let client_ident = format_ident!("{}Client", sanitize_ident(&interface.name).to_camel_case());

    let client = quote! {
        pub struct #client_ident {
            pub address: oasis_std::Address,
        }

        impl #client_ident {
            #client_impl
        }
    };

    let include_file = out_dir.join(format!("{}_client.rs", crate_name));
    std::fs::write(&include_file, &client.to_string()).unwrap();
    krate
        .module
        .items
        .push(super::common::gen_include_item(include_file));
}

fn build_wasi_client(service_def: &ServiceDefinition) -> proc_macro2::TokenStream {
    unimplemented!()
}

fn build_test_client(service_def: &ServiceDefinition) -> proc_macro2::TokenStream {
    unimplemented!()
}
