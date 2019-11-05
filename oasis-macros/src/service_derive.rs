#[proc_macro_derive(Service)]
pub fn service_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    if std::env::var("OASIS_BUILD_NO_SERVICE_DERIVE").is_ok() {
        return proc_macro::TokenStream::new();
    }
    let input = parse_macro_input!(input as syn::DeriveInput);
    let service = &input.ident;
    let impl_wrapper_ident = format_ident!("_IMPL_SERVICE_FOR_{}", service);
    proc_macro::TokenStream::from(match get_serde(&input) {
        Some((ser, de)) => {
            quote! {
                #[allow(warnings)]
                const #impl_wrapper_ident: () = {
                    use oasis_std::reexports::borsh::{BorshSerialize, BorshDeserialize};

                    impl oasis_std::exe::Service for #service {
                        fn coalesce() -> Self {
                            #de
                        }

                        fn sunder(service: Self) {
                            #ser
                        }
                    }
                };
            }
        }
        None => quote! {},
    })
}

fn get_serde(
    input: &syn::DeriveInput,
) -> Option<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
    if input.generics.type_params().count() > 0 {
        // early return because `impl Service` won't have generics which will
        // result in additional, confusing error messages.
        // No error because oasis-build will warn about this.
        return None;
    }

    let (named, fields) = match &input.data {
        syn::Data::Struct(s) => {
            let named = match &s.fields {
                syn::Fields::Named(_) | syn::Fields::Unit => true,
                syn::Fields::Unnamed(_) => false,
            };
            (named, s.fields.iter())
        }
        _ => {
            err!(input: "`#[derive(Service)]` can only be applied to structs.");
            return None;
        }
    };

    let (sers, des): (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) = fields
        .enumerate()
        .map(|(index, field)| {
            let (struct_idx, key) = match &field.ident {
                Some(ident) => (
                    syn::Member::Named(ident.clone()),
                    proc_macro2::Literal::string(&ident.to_string()),
                ),
                None => (
                    syn::Member::Unnamed(syn::Index {
                        index: index as u32,
                        span: proc_macro2::Span::call_site(),
                    }),
                    proc_macro2::Literal::string(&index.to_string()),
                ),
            };
            let (ser, de) = get_type_serde(&field.ty, struct_idx, key);
            let de = match &field.ident {
                Some(ident) => quote! { #ident: #de },
                None => de,
            };
            (ser, de)
        })
        .unzip();

    let ser = quote! { #(#sers);* };

    let de = if named {
        quote! { Self { #(#des),* } }
    } else {
        quote! { Self(#(#des),*) }
    };

    Some((ser, de))
}

/// Returns the serializer and deserializer for a Type.
fn get_type_serde(
    ty: &syn::Type,
    struct_idx: syn::Member,
    key: proc_macro2::Literal,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    use syn::Type::*;
    match ty {
        Group(g) => get_type_serde(&*g.elem, struct_idx, key),
        Paren(p) => get_type_serde(&*p.elem, struct_idx, key),
        Array(_) | Tuple(_) | Path(_) => (
            quote! {
                oasis_std::backend::write(
                    #key.as_bytes(),
                    &service.#struct_idx.try_to_vec().unwrap()
                )
            },
            quote! {
                BorshDeserialize::try_from_slice(
                    &oasis_std::backend::read(#key.as_bytes())
                ).unwrap()
            },
        ),
        ty => {
            err!(ty: "Service field must be a POD type.");
            (quote!(unreachable!()), quote!(unreachable!()))
        }
    }
}
