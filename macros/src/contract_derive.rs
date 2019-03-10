#[proc_macro_derive(Contract)]
pub fn contract_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let contract = &input.ident;

    let empty_punct = syn::punctuated::Punctuated::<_, syn::Token![,]>::new();
    let (named, fields) = match &input.data {
        syn::Data::Struct(syn::DataStruct { fields, .. }) => match fields {
            syn::Fields::Named(syn::FieldsNamed { named, .. }) => (true, named.iter()),
            syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, .. }) => (false, unnamed.iter()),
            syn::Fields::Unit => (true, empty_punct.iter()),
        },
        _ => {
            emit_err!(
                input,
                "`#[derive(Contract)]` can only be applied to structs."
            );
            return proc_macro::TokenStream::from(quote!());
        }
    };

    match input.vis {
        syn::Visibility::Public(_) => {}
        _ => emit_err!(
            input.vis,
            format!("`struct {}` should have `pub` visibility.", contract)
        ),
    }

    if input.generics.type_params().count() > 0 {
        emit_err!(input.generics, "Contract cannot contain generic types.")
    }

    let (sers, des): (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) = fields
        .cloned()
        .enumerate()
        .map(|(i, field)| {
            match field.vis {
                syn::Visibility::Inherited => {}
                _ => emit_warning!(field, "Field should have no visibility marker."),
            }
            get_type_serde(i, field.ident.as_ref(), &field.ty)
        })
        .unzip();

    let des = if named {
        quote! { Self { #(#des),* } }
    } else {
        quote! { Self(#(#des),*) }
    };

    proc_macro::TokenStream::from(quote! {
        impl Contract for #contract {
            fn coalesce() -> Self {
                #des
            }

            fn sunder(contract: Self) {
                #(#sers);*
            }
        }
    })
}

/// Returns the serializer and deserializer for a (possibly lazy) Type
fn get_type_serde(
    index: usize,
    field: Option<&syn::Ident>,
    ty: &syn::Type,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    use syn::Type::*;
    let key = match field {
        Some(ident) => keccak_key(ident),
        None => quote! { H256::from(#index) },
    };
    match ty {
        Group(g) => get_type_serde(index, field, &*g.elem),
        Paren(p) => get_type_serde(index, field, &*p.elem),
        Array(_) | Tuple(_) => default_serde(field, &key),
        Path(syn::TypePath { path, .. }) => {
            if path
                .segments
                .last()
                .map(|punct| punct.value().ident == parse_quote!(Lazy): syn::Ident)
                .unwrap_or(false)
            {
                let de = quote! { oasis_std::exe::Lazy::_uninitialized(#key) };
                (
                    quote! {
                        if contract.#field.is_initialized() {
                            oasis::set_bytes(
                                &#key,
                                &serde_cbor::to_vec(contract.#field.get()).unwrap()
                            ).unwrap()
                        }
                    },
                    match field {
                        Some(ident) => quote! { #ident: #de },
                        None => de,
                    },
                )
            } else {
                default_serde(field, &key)
            }
        }
        ty => {
            emit_err!(ty, "Contract field must be a POD type.");
            (quote!(compile_error!()), quote!(compile_error!()))
        }
    }
}

/// Returns the default serializer and deserializer for a struct field.
fn default_serde(
    field: Option<&syn::Ident>,
    key: &proc_macro2::TokenStream,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let de = quote! { serde_cbor::from_slice(&oasis::get_bytes(&#key).unwrap()).unwrap() };
    (
        quote! {
            oasis::set_bytes(
                &#key,
                &serde_cbor::to_vec(&contract.#field).unwrap()
            ).unwrap()
        },
        match field {
            Some(ident) => quote! { #ident: #de },
            None => de,
        },
    )
}
