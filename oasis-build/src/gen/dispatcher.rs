use std::{collections::BTreeSet, path::Path};

use proc_macro2::TokenStream;
use quote::quote;
use syntax::{
    ast::{self, Crate, ItemKind, StmtKind},
    print::pprust,
};
use syntax_pos::symbol::Symbol;

use crate::{
    format_ident,
    visitor::parsed_rpc::{ParsedRpc, ParsedRpcKind},
    BuildContext,
};

use super::{common, ServiceDefinition};

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

    let default_fn = rpcs.iter().find(|rpc| match rpc.kind {
        ParsedRpcKind::Default(_) => true,
        _ => false,
    });

    if !rpcs.is_empty() {
        let rpcs_dispatcher = generate_rpc_dispatcher(*service_name, &rpcs, default_fn);
        let rpcs_include_file = out_dir.join(format!("{}_dispatcher.rs", crate_name));
        common::write_include(&rpcs_include_file, &rpcs_dispatcher.to_string());
        insert_rpc_dispatcher_stub(krate, &rpcs_include_file);
    }

    let ctor_fn = generate_ctor_fn(*service_name, &ctor);
    let ctor_include_file = out_dir.join(format!("{}_ctor.rs", crate_name));
    common::write_include(&ctor_include_file, &ctor_fn.to_string());
    krate
        .module
        .items
        .push(common::gen_include_item(ctor_include_file));
}

fn generate_rpc_dispatcher(
    service_name: Symbol,
    rpcs: &[ParsedRpc],
    default_fn: Option<&ParsedRpc>,
) -> TokenStream {
    let service_ident = format_ident!("{}", service_name.as_str().get());
    let mut any_rpc_returns_result = false;
    let mut rpc_payload_variants = Vec::with_capacity(rpcs.len());
    let mut rpc_payload_lifetimes = BTreeSet::new();
    let rpc_match_arms = rpcs
        .iter()
        .map(|rpc| {
            let mut arg_tys = Vec::new();
            let mut needs_borrow = false;
            for (arg_lifetimes, arg_ty) in rpc.arg_lifetimes() {
                arg_tys.push(ty_tokenizable(&arg_ty));
                needs_borrow |= !arg_lifetimes.is_empty();
                rpc_payload_lifetimes.extend(arg_lifetimes.into_iter().map(lifetime_tokenizable));
            }

            let variant_arg_tys = if !arg_tys.is_empty() {
                quote!((#(#arg_tys),*,))
            } else {
                quote!()
            };

            let serde_borrow = if needs_borrow {
                quote!(#[serde(borrow)])
            } else {
                quote!()
            };

            let rpc_name = format_ident!("{}", rpc.name);
            rpc_payload_variants.push(quote! {
                #serde_borrow
                #rpc_name(#variant_arg_tys)
            });

            any_rpc_returns_result |= rpc.output.is_result();
            DispatchArm::new(&service_ident, &rpc)
        })
        .collect::<Vec<_>>();

    let default_fn_invocation = if let Some(rpc) = default_fn {
        let default_dispatch = DispatchArm::new(&service_ident, &rpc).invocation;
        quote! {
            if input.is_empty() {
                #default_dispatch;
            }
        }
    } else {
        quote!()
    };

    let output_err_ty = if any_rpc_returns_result {
        quote!(Vec<u8>)
    } else {
        quote!(())
    };

    let err_returner = if any_rpc_returns_result {
        quote!(oasis_std::backend::err(&err_output))
    } else {
        quote!(unreachable!("No RPC function returns Err"))
    };

    let rpc_payload_lifetimes = if !rpc_payload_lifetimes.is_empty() {
        quote!(<#(#rpc_payload_lifetimes),*>)
    } else {
        quote!()
    };

    quote! {
        #[allow(warnings)]
        fn _oasis_dispatcher() {
            use oasis_std::{Service as _, reexports::serde::Deserialize};

            #[derive(Deserialize)]
            #[serde(tag = "method", content = "payload")]
            enum RpcPayload#rpc_payload_lifetimes {
                #(#rpc_payload_variants),*
            }

            let ctx = oasis_std::Context::default(); // TODO(#33)
            let mut service = <#service_ident>::coalesce();
            let input = oasis_std::backend::input();
            #default_fn_invocation
            let payload: RpcPayload =
                oasis_std::reexports::serde_cbor::from_slice(&input).unwrap();
            let output: std::result::Result<Vec<u8>, #output_err_ty> = match payload {
                #(#rpc_match_arms)*
            };
            match output {
                Ok(output) => oasis_std::backend::ret(&output),
                Err(err_output) => #err_returner,
            }
        }
    }
}

mod armery {
    use super::*;

    pub struct DispatchArm {
        pub guard: TokenStream,
        pub invocation: TokenStream,
        sunderer: Option<TokenStream>,
    }

    impl DispatchArm {
        pub fn new(service_ident: &syn::Ident, rpc: &ParsedRpc) -> Self {
            let fn_name = format_ident!("{}", rpc.name);
            let arg_names: Vec<_> = rpc
                .arg_names()
                .map(|name| format_ident!("{}", name))
                .collect();
            let invocation = if rpc.output.is_result() {
                quote! {
                    match service.#fn_name(&ctx, #(#arg_names),*) {
                        Ok(output) => Ok(oasis_std::reexports::serde_cbor::to_vec(&output).unwrap()),
                        Err(err) => Err(oasis_std::reexports::serde_cbor::to_vec(&err).unwrap()),
                    }
                }
            } else {
                quote! {
                    Ok(oasis_std::reexports::serde_cbor::to_vec(
                            &service.#fn_name(&ctx, #(#arg_names),*)
                    ).unwrap())
                }
            };
            let variant_args = if !arg_names.is_empty() {
                quote!((#(#arg_names),*,))
            } else {
                quote!()
            };
            Self {
                guard: quote!(RpcPayload::#fn_name(#variant_args)),
                invocation,
                sunderer: if rpc.is_mut() {
                    Some(quote!(<#service_ident>::sunder(service);))
                } else {
                    None
                },
            }
        }
    }

    impl quote::ToTokens for DispatchArm {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let DispatchArm {
                guard, invocation, ..
            } = self;
            tokens.extend(match &self.sunderer {
                Some(sunderer) => {
                    quote! {
                        #guard => {
                            let output = #invocation;
                            #sunderer
                            output
                        }
                    }
                }
                None => quote!(#guard => { #invocation }),
            });
        }
    }
}
use armery::DispatchArm;

fn generate_ctor_fn(service_name: Symbol, ctor: &ParsedRpc) -> TokenStream {
    let arg_names: Vec<_> = ctor
        .arg_names()
        .map(|name| format_ident!("{}", name))
        .collect();
    let mut lifetimes = Vec::new();
    let mut payload_arg_tys = Vec::new();
    for (arg_lifetimes, arg_ty) in ctor.arg_lifetimes() {
        payload_arg_tys.push(ty_tokenizable(&arg_ty));
        lifetimes.extend(arg_lifetimes.into_iter().map(lifetime_tokenizable));
    }
    let payload_lifetimes = if !lifetimes.is_empty() {
        quote!(<#(#lifetimes),*>)
    } else {
        quote!()
    };

    let serde_borrow = if !lifetimes.is_empty() {
        quote!(#[serde(borrow)])
    } else {
        quote!()
    };

    let (ctor_struct_args, ctor_payload_unpack) = if !arg_names.is_empty() {
        let struct_args = quote!((#(#payload_arg_tys),*,));
        let payload_unpack = quote! {
            let input = oasis_std::backend::input();
            let CtorPayload((#(#arg_names),*,)) =
                oasis_std::reexports::serde_cbor::from_slice(&input).unwrap();
        };
        (struct_args, payload_unpack)
    } else {
        (quote!(), quote!())
    };

    let service_ident = format_ident!("{}", service_name.as_str().get());

    let ctor_stmt = if ctor.output.is_result() {
        quote! {
            match <#service_ident>::new(&ctx, #(#arg_names),*) {
                Ok(service) => service,
                Err(err) => {
                    oasis_std::backend::err(&format!("{:#?}", err).into_bytes());
                    return 1;
                }
            }
        }
    } else {
        quote! { <#service_ident>::new(&ctx, #(#arg_names),*) }
    };

    quote! {
        #[allow(warnings)]
        #[no_mangle]
        extern "C" fn _oasis_deploy() -> u8 {
            use oasis_std::{Service as _, reexports::serde::Deserialize};

            #[derive(Deserialize)]
            #[allow(non_camel_case_types)]
            struct CtorPayload#payload_lifetimes(#serde_borrow #ctor_struct_args);

            let ctx = oasis_std::Context::default(); // TODO(#33)
            #ctor_payload_unpack
            let mut service = #ctor_stmt;
            <#service_ident>::sunder(service);
            return 0;
        }
    }
}

fn insert_rpc_dispatcher_stub(krate: &mut Crate, include_file: &Path) {
    krate
        .module
        .items
        .push(common::gen_include_item(include_file));
    for item in krate.module.items.iter_mut() {
        if item.ident.name != Symbol::intern("main") {
            continue;
        }
        let main_fn_block = match &mut item.node {
            ItemKind::Fn(_, _, _, ref mut block) => block,
            _ => continue,
        };
        let oasis_macro_idx = main_fn_block
            .stmts
            .iter()
            .position(|stmt| match &stmt.node {
                StmtKind::Mac(p_mac) => {
                    crate::utils::path_ends_with(&p_mac.0.path, &["oasis_std", "service"])
                }
                _ => false,
            })
            .unwrap();
        main_fn_block.stmts.splice(
            oasis_macro_idx..=oasis_macro_idx,
            std::iter::once(common::gen_call_stmt(
                syntax::source_map::symbol::Ident::from_str("_oasis_dispatcher"),
            )),
        );
        break;
    }
}

fn ty_tokenizable(ty: &ast::Ty) -> syn::Type {
    syn::parse_str::<syn::Type>(&pprust::ty_to_string(&ty)).unwrap()
}

fn lifetime_tokenizable(name: Symbol) -> syn::Lifetime {
    syn::Lifetime::new(&format!("{}", name), proc_macro2::Span::call_site())
}
