use std::{path::Path, str::FromStr};

use mantle_rpc::import::ImportedService;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use proc_quote::quote;

macro_rules! format_ident {
    ($fmt_str:literal, $($fmt_arg:expr),+) => {
        Ident::new(&format!($fmt_str, $($fmt_arg),+), Span::call_site())
    }
}

fn sanitize_ident(ident: &str) -> String {
    ident
        .chars()
        .filter(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '-')
        .collect()
}

pub fn generate(services: &[ImportedService], out_dir: &Path) {
    for service in services {
        let mod_name_str = sanitize_ident(&service.interface.namespace).to_snake_case();
        let mod_ident = format_ident!("{}", mod_name_str);

        let def_tys = gen_def_tys(&service.interface.type_defs);
        let client = gen_client(&service);

        let service_toks = quote! {
            mod #mod_ident {
                #(#def_tys)*

                #client
            }
        };

        std::fs::write(
            out_dir.join(format!("{}.rs", mod_name_str)),
            service_toks.to_string(),
        )
        .unwrap();
    }
}

fn gen_def_tys<'a>(defs: &'a Vec<mantle_rpc::TypeDef>) -> impl Iterator<Item = TokenStream> + 'a {
    defs.iter().map(|def| {
        let name = format_ident!("{}", def.name());
        match def {
            mantle_rpc::TypeDef::Struct { fields, .. } => {
                let is_newtype = fields
                    .iter()
                    .enumerate()
                    .all(|(i, f)| usize::from_str(&f.name) == Ok(i));
                let tys = fields.iter().map(|f| quote_ty(&f.ty));
                if is_newtype {
                    quote! {
                        pub struct #name(#(#tys),*)
                    }
                } else {
                    let field_names = fields.iter().map(|f| format_ident!("{}", f.name));
                    quote! {
                        #[derive(Serialize, Deserialize)]
                        pub struct #name {
                            #(#field_names: #tys),*
                        }
                    }
                }
            }
            mantle_rpc::TypeDef::Enum { variants, .. } => {
                let variants = variants.iter().map(|v| format_ident!("{}", v));
                quote! {
                    #[derive(Serialize, Deserialize)]
                    pub enum #name {
                        #(#variants),*
                    }
                }
            }
            mantle_rpc::TypeDef::Event {
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
                    #[derive(Serialize, Deserialize, Event)]
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
        bytecode,
        interface,
    } = service;

    let client_ident = format_ident!("{}Client", sanitize_ident(&interface.name).to_camel_case());

    let def_tys = gen_def_tys(&interface.type_defs);

    let (rpcs, payload_variants): (TokenStream, TokenStream) =
        gen_rpcs(&interface.functions).unzip();
    let ctors = gen_ctors(&interface.constructor);

    quote! {
        pub struct #client_ident {
            pub address: mantle::Address,
        }

        #[derive(Serialize, Deserialize)]
        #[serde(tag = "method", content = "payload")]
        enum RpcPayload {
            #payload_variants
        }

        impl #client_ident {
            #ctors

            #(#rpcs)*
        }
    }
}

fn gen_rpcs<'a>(
    functions: &'a Vec<mantle_rpc::Function>,
) -> impl Iterator<Item = (TokenStream, TokenStream)> + 'a {
    functions.iter().map(|func| {
        let fn_name = format_ident!("{}", func.name);

        let self_ref = match func.mutability {
            mantle_rpc::StateMutability::Immutable => quote! { &self },
            mantle_rpc::StateMutability::Mutable => quote! { &mut self },
        };

        let (arg_names, arg_tys): (Vec<Ident>, Vec<TokenStream>) = func.inputs.iter().map(|field| {
            (format_ident!("{}", field.name), quote_borrow(&field.ty))
        }).unzip();

        let (output_ty, err_ty) = match &func.output {
            Some(mantle_rpc::Type::Result(ok_ty, err_ty)) => (quote_ty(ok_ty), quote_ty(err_ty)),
            Some(ty) => (quote_ty(ty), quote!(())),
            None => (quote!(()), quote!(())),
        };

        let rpc_toks = quote! {
            pub fn #fn_name(
                #self_ref,
                ctx: &mantle::Context,
                #(#arg_names: #arg_tys),*
           ) -> Result<#output_ty, mantle::RpcError<#err_ty>> {
                let payload = RpcPayload::#fn_name { #(#arg_names),* };
                #[cfg(target_os = "wasi")] {
                    mantle::transact(self.address, ctx.value.unwrap_or(0), &mantle::reexports::serde_cbor::to_vec(&payload).unwrap())
                }
                #[cfg(not(target_os = "wasi"))] {
                    compile_error!("Native client not yet implemented.")
                }
            }
        };

        let payload_variant = quote! {
            #fn_name {
                #(#arg_names: #arg_tys),*
            }
        };

        (rpc_toks, payload_variant)
    })
}

fn gen_ctors(ctor: &mantle_rpc::Constructor) -> TokenStream {
    let error_ty = if let Some(error) = &ctor.error {
        quote_ty(&error)
    } else {
        quote!()
    };
    let (arg_names, arg_tys): (Vec<Ident>, Vec<TokenStream>) = ctor
        .inputs
        .iter()
        .map(|inp| (format_ident!("{}", inp.name), quote_ty(&inp.ty)))
        .unzip();
    quote! {
        pub fn new(
            ctx: &Context,
            #(#arg_names: #arg_tys)*
        ) -> Result<Self, mantle::RpcError<#error_ty>> {
            unimplemented!()
        }

        pub fn at(address: mantle::Address) -> Self {
            Self {
                address
            }
        }
    }
}

fn quote_ty(ty: &mantle_rpc::Type) -> TokenStream {
    use mantle_rpc::Type;
    match ty {
        Type::Bool => quote!(bool),
        Type::U8 => quote!(u8),
        Type::I8 => quote!(i8),
        Type::U16 => quote!(u16),
        Type::I16 => quote!(i16),
        Type::U32 => quote!(u32),
        Type::I32 => quote!(i32),
        Type::U64 => quote!(u64),
        Type::I64 => quote!(i64),
        Type::F32 => quote!(f32),
        Type::F64 => quote!(f64),
        Type::Bytes => quote!(Vec<u8>),
        Type::String => quote!(&str),
        Type::Address => quote!(mantle::Address),
        Type::Defined { namespace, ty } => {
            let tyq = format_ident!("{}", ty);
            match namespace {
                Some(namespace) => {
                    let ns = format_ident!("{}", namespace);
                    quote!(#ns::#tyq)
                }
                None => quote!(#tyq),
            }
        }
        Type::Tuple(tys) => {
            let tyqs = tys.iter().map(quote_ty);
            quote!(( #(#tyqs),*) )
        }
        Type::Array(ty, count) => {
            let tyq = quote_ty(ty);
            let count = Literal::usize_suffixed(*count as usize);
            quote!([#tyq; #count])
        }
        Type::List(ty) => {
            let tyq = quote_ty(ty);
            quote!(Vec<#tyq>)
        }
        Type::Set(ty) => {
            let tyq = quote_ty(ty);
            quote!(std::collections::HashSet<#tyq>)
        }
        Type::Map(kty, vty) => {
            let ktyq = quote_ty(kty);
            let vtyq = quote_ty(vty);
            quote!(std::collections::HashMap<#ktyq, #vtyq>)
        }
        Type::Optional(ty) => {
            let tyq = quote_ty(ty);
            quote!(Option<#tyq>)
        }
        Type::Result(ok_ty, err_ty) => {
            let ok_tyq = quote_ty(ok_ty);
            let err_tyq = quote_ty(err_ty);
            quote!(Result<#ok_tyq, #err_tyq>)
        }
    }
}

fn quote_borrow(ty: &mantle_rpc::Type) -> TokenStream {
    use mantle_rpc::Type;
    let tyq = match ty {
        Type::Bool
        | Type::U8
        | Type::I8
        | Type::U16
        | Type::I16
        | Type::U32
        | Type::I32
        | Type::U64
        | Type::I64
        | Type::F32
        | Type::F64 => {
            return quote_ty(ty);
        }
        Type::Bytes => quote!([u8]),
        Type::String => quote!(str),
        Type::List(ty) => {
            let tyq = quote_ty(ty);
            quote!([#tyq])
        }
        _ => quote_ty(ty),
    };
    quote!(impl std::borrow::Borrow<#tyq>)
}

#[cfg(test)]
mod tests {
    #[test]
    fn sanitize_ident() {
        assert_eq!(&sanitize_ident("../../../../bin/bash"), "binbash");
        assert_eq!(&sanitize_ident("snake_case"), "snake_case");
        assert_eq!(&sanitize_ident("kebab-case"), "kebab-case");
        assert_eq!(&sanitize_ident(""), "");
        assert_eq!(&sanitize_ident(r#"!@#$%^&*()+=[]|\{}"'.,/<>?""#), "");
        assert_eq!(&sanitize_ident("˙´¬¬ø ∑ø®¬∂"), "");
        assert_eq!(&sanitize_ident(" \n\t\r"), "");
    }
}
