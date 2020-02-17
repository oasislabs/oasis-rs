use std::{path::Path, str::FromStr as _};

use colored::*;
use heck::{CamelCase as _, SnakeCase as _};
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
    import_name_loc: (String, ImportLocation),
    gen_dir: &Path,
    out_dir: &Path,
    mut rustc_args: Vec<String>,
) -> anyhow::Result<Import> {
    let service = resolve_imports(
        std::iter::once(import_name_loc),
        Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()),
    )?
    .pop()
    .unwrap();

    rustc_args.push("--crate-name".to_string());

    let interface_hash = hash!(service.interface, get_rustc_version());

    let mod_name = sanitize_ident(&service.interface.namespace).to_snake_case();
    let mod_path = gen_dir.join(format!("{}-{:016x}.rs", mod_name, interface_hash));
    let lib_path = out_dir.join(format!("lib{}-{:016x}.rlib", mod_name, interface_hash));

    let import = Import {
        name: mod_name.clone(),
        version: service.interface.version.clone(),
        lib_path: lib_path.clone(),
    };

    if lib_path.is_file() && std::env::var_os("OASIS_BUILD_NO_CACHE").is_none() {
        eprintln!(
            "       {} {name} v{version} ({path})",
            "Fresh".green(),
            name = mod_name,
            version = service.interface.version,
            path = mod_path.display()
        );
        return Ok(import);
    }

    let def_tys = gen_def_tys(&service.interface.type_defs);
    let client = gen_client(&service);

    let service_toks = quote! {
        #![allow(warnings)]

        extern crate oasis_std;

        use oasis_std::{abi::*, abi_encode, Address, AddressExt as _, Context, RpcError};

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

    if std::env::var("OASIS_BUILD_VERBOSE").is_ok() {
        eprintln!(
            "     {} `rustc {}`",
            "Running".green(),
            rustc_args.join(" ")
        );
    }

    rustc_driver::run_compiler(&rustc_args, &mut crate::DefaultCallbacks, None, None)
        .map_err(|_| anyhow::format_err!("Could not build `{}`", mod_name))?;

    Ok(import)
}

fn gen_def_tys<'a>(defs: &'a [oasis_rpc::TypeDef]) -> impl Iterator<Item = TokenStream> + 'a {
    defs.iter().map(|def| {
        let name = format_ident!("{}", def.name());
        let mut hashable_checker = HashableChecker::default();
        oasis_rpc::visitor::IdlVisitor::visit_type_def(&mut hashable_checker, def);
        let hash_derive = if hashable_checker.is_hash() {
            quote!(Hash)
        } else {
            quote!()
        };
        let derives = quote!(Serialize, Deserialize, Debug, Clone, PartialEq, #hash_derive);
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

fn gen_client(service: &ImportedService) -> TokenStream {
    let ImportedService {
        interface,
        bytecode,
    } = service;

    let client_ident = format_ident!("{}Client", sanitize_ident(&interface.name).to_camel_case());

    let rpcs = gen_rpcs(&interface.functions).collect::<Vec<_>>();

    let service_bytecode = quote!(&[#(#bytecode),*]); // TODO(#247)

    let (ctor_arg_names, ctor_arg_tys): (Vec<Ident>, Vec<TokenStream>) = interface
        .constructor
        .inputs
        .iter()
        .map(|field| (format_ident!("{}", field.name), quote_borrow(&field.ty)))
        .unzip();

    quote! {
        #[cfg(target_os = "wasi")]
        mod client {
            use super::*;

            pub struct #client_ident {
                address: Address,
            }

            impl #client_ident {
                pub fn new(address: Address) -> Self {
                    Self {
                        address,
                    }
                }

                fn rpc(&self, ctx: &Context, payload: &[u8]) -> Result<Vec<u8>, RpcError> {
                    self.address.call(ctx, payload)
                }

                #(#rpcs)*
            }
        }

        #[cfg(not(target_os = "wasi"))]
        mod client {
            use super::*;

            use oasis_std::reexports::oasis_client::gateway::Gateway;

            pub struct #client_ident<'a> {
                address: Address,
                gateway: &'a dyn Gateway,
            }

            static SERVICE_BYTECODE: &[u8] = #service_bytecode;

            impl<'a> #client_ident<'a> {
                pub fn new(gateway: &'a dyn Gateway, address: Address) -> Self {
                    Self {
                        address,
                        gateway
                    }
                }

                pub fn deploy(
                    gateway: &'a dyn Gateway,
                    ctx: &Context,
                    #(#ctor_arg_names: #ctor_arg_tys),*
                ) -> Result<Self, RpcError> {
                    let mut initcode = SERVICE_BYTECODE.to_vec();
                    abi_encode!(#(#ctor_arg_names),* => &mut initcode)?;
                    Ok(Self {
                        address: gateway.deploy(&initcode)?,
                        gateway,
                    })
                }

                fn rpc(&self, ctx: &Context, payload: &[u8]) -> Result<Vec<u8>, RpcError> {
                    self.gateway.rpc(self.address, payload)
                }

                #(#rpcs)*
            }
        }

        pub use client::*;
    }
}

fn gen_rpcs<'a>(functions: &'a [oasis_rpc::Function]) -> impl Iterator<Item = TokenStream> + 'a {
    functions.iter().enumerate().map(|(func_idx, func)| {
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

        let output_ty = func.output.as_ref().map(quote_ty).unwrap_or_default();
        let (output_deserializer, err_deserializer) = match func.output.as_ref() {
            Some(oasis_rpc::Type::Result(box ok_ty, box err_ty)) => {
                let quot_ok_ty = quote_ty(ok_ty);
                let quot_err_ty = quote_ty(err_ty);
                let output_deserializer = quote! {
                    Ok(<#quot_ok_ty>::try_from_slice(&output)
                        .map_err(|_| oasis_std::RpcError::InvalidOutput(output))?)
                };
                let err_deserializer = quote! {
                    Err(<#quot_err_ty>::try_from_slice(&err_output)
                        .map_err(|_| oasis_std::RpcError::InvalidOutput(err_output))?)
                };
                (output_deserializer, err_deserializer)
            }
            Some(output_ty) => {
                let quot_output_ty = quote_ty(output_ty);
                let output_deserializer = quote! {
                    <#quot_output_ty>::try_from_slice(&output)
                        .map_err(|_| oasis_std::RpcError::InvalidOutput(output))?
                };
                (
                    output_deserializer,
                    quote!(Err(oasis_std::RpcError::Execution(err_output))?),
                )
            }
            None => (
                quote!(()),
                quote!(Err(oasis_std::RpcError::Execution(err_output))?),
            ),
        };

        quote! {
            pub fn #fn_name(
                #self_ref,
                ctx: &oasis_std::Context,
                #(#arg_names: #arg_tys),*
           ) -> Result<#output_ty, oasis_std::RpcError> {
                let payload = abi_encode!(#func_idx as u8, #(#arg_names),*).unwrap();
                match self.rpc(ctx, &payload) {
                    Ok(output) => {
                        Ok(#output_deserializer)
                    }
                    Err(oasis_std::RpcError::Execution(err_output)) => {
                        Ok(#err_deserializer)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    })
}

fn get_rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .expect("Could not determine rustc version")
}

#[derive(Default)]
struct HashableChecker {
    contains_nonhashable: bool,
}

impl HashableChecker {
    fn is_hash(&self) -> bool {
        !self.contains_nonhashable
    }
}

impl oasis_rpc::visitor::IdlVisitor for HashableChecker {
    fn visit_type(&mut self, ty: &oasis_rpc::Type) {
        use oasis_rpc::Type::{F32, F64};
        self.contains_nonhashable |= *ty == F32 || *ty == F64;
        oasis_rpc::visitor::walk_type(self, ty);
    }
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
