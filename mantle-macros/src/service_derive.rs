#[proc_macro_derive(Service)]
pub fn service_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let service = &input.ident;
    proc_macro::TokenStream::from(match get_serde(&input) {
        Some((ser, de)) => {
            quote! {
                impl mantle::exe::Service for #service {
                    fn coalesce() -> Self {
                        #de
                    }

                    fn sunder(service: Self) {
                        #ser
                    }
                }
            }
        }
        None => quote! {},
    })
}

fn get_serde(
    input: &syn::DeriveInput,
) -> Option<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
    if input.generics.type_params().count() > 0 {
        err!(input.generics: "Service cannot contain generic types.");
        // early return because `impl Service` won't have generics which will
        // result in additional, confusing error messages.
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
                Some(ident) => (parse_quote!(#ident): syn::Member, keccak_key(ident)),
                None => {
                    // this is a hack for rustc nightly which quotes a bogus suffix onto index
                    let struct_index: proc_macro2::TokenStream = quote! { #index }
                        .into_iter()
                        .map(|itm| match itm {
                            proc_macro2::TokenTree::Literal(_) => {
                                proc_macro2::Literal::usize_unsuffixed(index).into()
                            }
                            _ => itm,
                        })
                        .collect();
                    (
                        parse_quote!(#struct_index): syn::Member,
                        quote! { &#index.to_le_bytes() },
                    )
                }
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
    key: proc_macro2::TokenStream,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    use syn::Type::*;
    match ty {
        Group(g) => get_type_serde(&*g.elem, struct_idx, key),
        Paren(p) => get_type_serde(&*p.elem, struct_idx, key),
        Array(_) | Tuple(_) | Path(_) => (
            quote! {
                mantle::ext::write(
                    &#key,
                    &serde_cbor::to_vec(&service.#struct_idx).unwrap()
                ).unwrap()
            },
            quote! { serde_cbor::from_slice(&mantle::ext::read(&#key).unwrap()).unwrap() },
        ),
        ty => {
            err!(ty: "Service field must be a POD type.");
            (quote!(unreachable!()), quote!(unreachable!()))
        }
    }
}
