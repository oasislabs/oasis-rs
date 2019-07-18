use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash as _, Hasher as _},
    path::Path,
    str::FromStr,
};

use colored::*;
use heck::SnakeCase;
use proc_macro2::{Ident, Span, TokenStream};
use proc_quote::quote;

use super::{
    common::{quote_ty, sanitize_ident},
    format_ident, generate_client,
};

pub struct Import {
    pub name: String,
    pub version: String,
    pub lib_path: std::path::PathBuf,
}

pub fn build(
    top_level_deps: impl IntoIterator<Item = (String, String)>,
    gen_dir: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    mut rustc_args: Vec<String>,
) -> Result<Vec<Import>, failure::Error> {
    let out_dir = out_dir.as_ref();

    let services = oasis_rpc::import::Resolver::new(
        top_level_deps.into_iter().collect(),
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()),
    )
    .resolve()?;

    let mut imports = Vec::with_capacity(services.len());

    rustc_args.push("--crate-name".to_string());

    for service in services {
        let mut hasher = DefaultHasher::new();
        service.interface.hash(&mut hasher);
        let interface_hash = hasher.finish();

        let mod_name = sanitize_ident(&service.interface.namespace).to_snake_case();
        let mod_path = gen_dir.as_ref().join(format!("{}.rs", mod_name));
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
        let client = generate_client(&service);

        let service_toks = quote! {
            #![allow(warnings)]

            #[macro_use]
            extern crate serde;

            #(#def_tys)*

            #client
        };

        std::fs::write(&mod_path, service_toks.to_string())
            .unwrap_or_else(|err| panic!("Could not generate `{}`: {}", mod_name, err));

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
                let variants = variants.iter().map(|v| format_ident!("{}", v));
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
                    #[derive(#derives, Event)]
                    pub struct #name {
                        #(#indexeds #field_names: #tys),*
                    }
                }
            }
        }
    })
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
