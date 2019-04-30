#[proc_macro_derive(Contract)]
pub fn contract_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let contract = &input.ident;
    proc_macro::TokenStream::from(match get_serde(&input) {
        Ok((ser, de)) => {
            quote! {
                impl oasis_std::exe::Contract for #contract {
                    fn coalesce() -> Self {
                        #de
                    }

                    fn sunder(contract: Self) {
                        #ser
                    }
                }
            }
        }
        Err(_) => quote! {},
    })
}

fn get_serde(
    input: &syn::DeriveInput,
) -> Result<(proc_macro2::TokenStream, proc_macro2::TokenStream), ()> {
    if input.generics.type_params().count() > 0 {
        err!(input.generics: "Contract cannot contain generic types.");
        // early return because `impl Contract` won't have generics which will
        // result in additional, confusing error messages.
        return Err(());
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
            err!(input: "`#[derive(Contract)]` can only be applied to structs.");
            return Err(());
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
                        quote! { H256::from(#index as u32) },
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

    Ok((ser, de))
}

/// Returns the serializer and deserializer for a (possibly lazy) Type.
fn get_type_serde(
    ty: &syn::Type,
    struct_idx: syn::Member,
    key: proc_macro2::TokenStream,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    use syn::Type::*;
    match ty {
        Group(g) => get_type_serde(&*g.elem, struct_idx, key),
        Paren(p) => get_type_serde(&*p.elem, struct_idx, key),
        Array(_) | Tuple(_) => default_serde(&struct_idx, &key),
        Path(syn::TypePath { path, .. }) => {
            if path
                .segments
                .last()
                .map(|seg| seg.value().ident == parse_quote!(Lazy): syn::Ident)
                .unwrap_or(false)
            {
                (
                    quote! {
                        if contract.#struct_idx.is_initialized() {
                            oasis::set_bytes(
                                &#key,
                                &serde_cbor::to_vec(contract.#struct_idx.get()).unwrap()
                            ).unwrap()
                        }
                    },
                    quote! { oasis_std::exe::Lazy::_uninitialized(#key) },
                )
            } else {
                default_serde(&struct_idx, &key)
            }
        }
        ty => {
            err!(ty: "Contract field must be a POD type.");
            (quote!(unreachable!()), quote!(unreachable!()))
        }
    }
}

/// Returns the default serializer and deserializer for a struct field.
fn default_serde(
    struct_idx: &syn::Member,
    key: &proc_macro2::TokenStream,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    (
        quote! {
            oasis::set_bytes(
                &#key,
                &serde_cbor::to_vec(&contract.#struct_idx).unwrap()
            ).unwrap()
        },
        quote! { serde_cbor::from_slice(&oasis::get_bytes(&#key).unwrap()).unwrap() },
    )
}
