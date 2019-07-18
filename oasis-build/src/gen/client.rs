use heck::CamelCase;
use oasis_rpc::import::ImportedService;
use proc_macro2::{Ident, Span, TokenStream};
use proc_quote::quote;

use super::{
    common::{quote_borrow, quote_ty, sanitize_ident},
    format_ident,
};

pub fn generate(service: &ImportedService) -> TokenStream {
    let ImportedService {
        bytecode,
        interface,
    } = service;

    let client_ident = format_ident!("{}Client", sanitize_ident(&interface.name).to_camel_case());

    let rpcs = gen_rpcs(&interface.functions);
    let ctor_fns = gen_ctors(&interface.constructor, &bytecode);

    quote! {
        pub struct #client_ident {
            pub address: oasis_std::Address,
        }

        impl #client_ident {
            #ctor_fns

            #(#rpcs)*
        }
    }
}

fn gen_rpcs<'a>(functions: &'a [oasis_rpc::Function]) -> impl Iterator<Item = TokenStream> + 'a {
    functions.iter().enumerate().map(|(fn_idx, func)| {
        let fn_name = format_ident!("{}", func.name);

        let self_ref = match func.mutability {
            oasis_rpc::StateMutability::Immutable => quote! { &self },
            oasis_rpc::StateMutability::Mutable => quote! { &mut self },
        };

        let num_args = func.inputs.len();
        let (arg_names, arg_tys): (Vec<Ident>, Vec<TokenStream>) = func
            .inputs
            .iter()
            .map(|field| (format_ident!("{}", field.name), quote_borrow(&field.ty)))
            .unzip();

        let (output_ty, err_ty) = match &func.output {
            Some(oasis_rpc::Type::Result(ok_ty, err_ty)) => (quote_ty(ok_ty), quote_ty(err_ty)),
            Some(ty) => (quote_ty(ty), quote!(())),
            None => (quote!(()), quote!(())),
        };

        quote! {
            pub fn #fn_name(
                #self_ref,
                ctx: &oasis_std::Context,
                #(#arg_names: #arg_tys),*
           ) -> Result<#output_ty, oasis_std::RpcError<#err_ty>> {
                use serde::ser::{Serializer as _, SerializeTupleVariant as _};
                let mut serializer = oasis_std::reexports::serde_cbor::Serializer::new(Vec::new());
                let mut state = serializer.serialize_tuple_variant(
                    "" /* unused enum name */,
                    #fn_idx as u32 /* unused */,
                    stringify!(#fn_name),
                    #num_args
                ) .unwrap();
                #(state.serialize_field(#arg_names.borrow()).unwrap();)*
                state.end().unwrap();
                let payload = serializer.into_inner();

                #[cfg(target_os = "wasi")] {
                    let output = oasis_std::backend::transact(
                        &self.address,
                        ctx.value.unwrap_or(0),
                        &oasis_std::reexports::serde_cbor::to_vec(&payload).unwrap()
                    )?;
                    Ok(oasis_std::reexports::serde_cbor::from_slice(&output)
                       .map_err(|_| oasis_std::RpcError::InvalidOutput(output))?)
                }
                #[cfg(not(target_os = "wasi"))] {
                    unimplemented!("Native client not yet implemented.")
                }
            }
        }
    })
}

fn gen_ctors(ctor: &oasis_rpc::Constructor, _bytecode: &[u8]) -> TokenStream {
    let (arg_names, arg_tys): (Vec<Ident>, Vec<TokenStream>) = ctor
        .inputs
        .iter()
        .map(|inp| (format_ident!("{}", inp.name), quote_ty(&inp.ty)))
        .unzip();

    let error_ty = if let Some(error) = &ctor.error {
        quote_ty(&error)
    } else {
        quote!(())
    };

    quote! {
        pub fn new(
            ctx: &oasis_std::Context,
            #(#arg_names: #arg_tys)*
        ) -> Result<Self, oasis_std::RpcError<#error_ty>> {
            unimplemented!()
        }

        pub fn at(address: oasis_std::Address) -> Self {
            Self {
                address
            }
        }
    }
}
