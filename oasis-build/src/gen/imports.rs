use std::{path::Path, str::FromStr};

use colored::*;
use heck::{CamelCase, SnakeCase};
use oasis_rpc::import::{resolve_imports, ImportLocation, ImportedService};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::{format_ident, hash};

use super::common::{quote_borrow, quote_ty, sanitize_ident, write_generated};

pub struct Import {
    pub name: String,
    pub version: String,
    pub lib_path: std::path::PathBuf,
}

pub fn build(
    top_level_deps: impl IntoIterator<Item = (String, ImportLocation)>, // name -
    gen_dir: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    mut rustc_args: Vec<String>,
) -> Result<Vec<Import>, failure::Error> {
    let out_dir = out_dir.as_ref();

    let services = resolve_imports(
        top_level_deps,
        Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()),
    )?;

    let mut imports = Vec::with_capacity(services.len());

    rustc_args.push("--crate-name".to_string());

    for service in services {
        let interface_hash = hash!(service.interface, get_rustc_version());

        let mod_name = sanitize_ident(&service.interface.namespace).to_snake_case();
        let mod_path = gen_dir
            .as_ref()
            .join(format!("{}-{:016x}.rs", mod_name, interface_hash));
        let lib_path = out_dir.join(format!("lib{}-{:016x}.rlib", mod_name, interface_hash));

        imports.push(Import {
            name: mod_name.clone(),
            version: service.interface.version.clone(),
            lib_path: lib_path.clone(),
        });

        if lib_path.is_file() {
            eprintln!(
                "       {} {name} v{version} ({path})",
                "Fresh".green(),
                name = mod_name,
                version = service.interface.version,
                path = mod_path.display()
            );
            continue;
        }

        let def_tys = gen_def_tys(&service.interface.type_defs);
        let client = gen_client(&service);

        let service_toks = quote! {
            #![allow(warnings)]

            #[macro_use]
            extern crate serde;
            
            use std::borrow::Borrow;

            #(#def_tys)*

            #client
        };

        write_generated(&mod_path, &service_toks.to_string());

        eprintln!(
            "   {} {name} v{version} ({path})",
            "Compiling".green(),
            name = mod_name,
            version = service.interface.version,
            path = mod_path.display()
        );
        rustc_args.push(mod_name.clone());
        rustc_args.push(mod_path.display().to_string());
        rustc_args.push(format!("-Cextra-filename=-{:016x}", interface_hash));
        rustc_driver::run_compiler(&rustc_args, &mut rustc_driver::DefaultCallbacks, None, None)
            .map_err(|_| failure::format_err!("Could not build `{}`", mod_name))?;
        rustc_args.pop();
        rustc_args.pop();
        rustc_args.pop();
    }
    Ok(imports)
}

fn gen_def_tys<'a>(defs: &'a [oasis_rpc::TypeDef]) -> impl Iterator<Item = TokenStream> + 'a {
    defs.iter().map(|def| {
        let name = format_ident!("{}", def.name());
        let derives = quote!(Serialize, Deserialize, Debug, Clone, PartialEq, Hash);
        match def {
            oasis_rpc::TypeDef::Struct { fields, .. } => {
                let is_newtype = fields
                    .iter()
                    .enumerate()
                    .all(|(i, f)| usize::from_str(&f.name) == Ok(i));
                let tys = fields.iter().map(|f| quote_ty(&f.ty));
                if is_newtype {
                    quote! {
                        #[derive(#derives)]
                        pub struct #name(#(pub #tys),*);
                    }
                } else {
                    let field_names = fields.iter().map(|f| format_ident!("{}", f.name));
                    quote! {
                        #[derive(#derives)]
                        pub struct #name {
                            #(pub #field_names: #tys),*
                        }
                    }
                }
            }
            oasis_rpc::TypeDef::Enum { variants, .. } => {
                let variants = variants.iter().map(|v| {
                    let name = format_ident!("{}", v.name);
                    match &v.fields {
                        Some(oasis_rpc::EnumFields::Named(fields)) => {
                            let field_names = fields.iter().map(|f| format_ident!("{}", f.name));
                            let tys = fields.iter().map(|f| quote_ty(&f.ty));
                            quote! {
                                #name {
                                    #(#field_names: #tys),*
                                }
                            }
                        }
                        Some(oasis_rpc::EnumFields::Tuple(tys)) => {
                            let tys = tys.iter().map(quote_ty);
                            quote!(#name(#(#tys),*))
                        }
                        None => quote!(#name),
                    }
                });
                quote! {
                    #[derive(#derives)]
                    pub enum #name {
                        #(#variants),*
                    }
                }
            }
            oasis_rpc::TypeDef::Event {
                fields: indexed_fields,
                ..
            } => {
                let field_names = indexed_fields.iter().map(|f| format_ident!("{}", f.name));
                let tys = indexed_fields.iter().map(|f| quote_ty(&f.ty));
                let indexeds = indexed_fields.iter().map(|f| {
                    if f.indexed {
                        quote!(#[indexed])
                    } else {
                        quote!()
                    }
                });
                quote! {
                    #[derive(#derives, oasis_std::Event)]
                    pub struct #name {
                        #(#indexeds #field_names: #tys),*
                    }
                }
            }
        }
    })
}

pub fn gen_client(service: &ImportedService) -> TokenStream {
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
    functions.iter().map(|func| {
        let fn_name = format_ident!("{}", func.name);

        let self_ref = match func.mutability {
            oasis_rpc::StateMutability::Immutable => quote! { &self },
            oasis_rpc::StateMutability::Mutable => quote! { &mut self },
        };

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

        let (payload_ty, payload_def) = if arg_names.is_empty() {
            (quote!(&[()]), quote!(&[]))
        } else {
            (quote!(_), quote!(&(#(#arg_names.borrow()),*,)))
        };

        quote! {
            pub fn #fn_name(
                #self_ref,
                ctx: &oasis_std::Context,
                #(#arg_names: #arg_tys),*
           ) -> Result<#output_ty, oasis_std::RpcError<#err_ty>> {
                use std::borrow::Borrow as _;
                use serde::ser::{Serializer as _, SerializeStruct as _};

                let mut serializer = oasis_std::reexports::serde_cbor::Serializer::new(Vec::new());
                let mut sstruct = serializer.serialize_struct("RpcPayload", 2).unwrap();
                sstruct.serialize_field("method", stringify!(#fn_name)).unwrap();
                let payload: #payload_ty = #payload_def;
                sstruct.serialize_field("payload", payload).unwrap();
                sstruct.end().unwrap();
                let payload = serializer.into_inner();

                #[cfg(target_os = "wasi")] {
                    let output = oasis_std::backend::transact(
                        &self.address,
                        ctx.value.unwrap_or_default(),
                        &payload,
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

fn get_rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .expect("Could not determine rustc version")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_ident() {
        assert_eq!(&sanitize_ident("../../../../bin/bash"), "binbash");
        assert_eq!(&sanitize_ident("snake_case"), "snake_case");
        assert_eq!(&sanitize_ident("kebab-case"), "kebab-case");
        assert_eq!(&sanitize_ident(""), "");
        assert_eq!(&sanitize_ident(r#"!@#$%^&*()+=[]|\{}"'.,/<>?""#), "");
        assert_eq!(&sanitize_ident("˙´¬¬ø ∑ø®¬∂"), "");
        assert_eq!(&sanitize_ident(" \n\t\r"), "");
    }
}
