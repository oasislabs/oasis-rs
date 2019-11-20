#[proc_macro_derive(Event, attributes(indexed))]
pub fn event_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let event_name = &input.ident;
    let generics = &input.generics;

    let fields = match input.data {
        syn::Data::Struct(syn::DataStruct { fields, .. }) => fields,
        _ => {
            err!(input: "An `Event` must be a struct.");
            return proc_macro::TokenStream::new();
        }
    };

    fn is_indexed(field: &syn::Field) -> bool {
        field.attrs.iter().any(|attr| attr.path.is_ident("indexed"))
    }

    let indexed_field_idents = match fields {
        syn::Fields::Named(syn::FieldsNamed { named, .. }) => named
            .iter()
            .filter_map(|field| {
                if is_indexed(field) {
                    Some(syn::Member::Named(field.ident.as_ref().unwrap().clone()))
                } else {
                    None
                }
            })
            .collect(),
        syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, .. }) => unnamed
            .iter()
            .enumerate()
            .filter_map(|(i, field)| {
                if is_indexed(field) {
                    Some(syn::Member::Unnamed(syn::Index {
                        index: i as u32,
                        span: proc_macro2::Span::call_site(),
                    }))
                } else {
                    None
                }
            })
            .collect(),
        syn::Fields::Unit => Vec::new(),
    };

    let impl_wrapper_ident = format_ident!("_IMPL_EVENT_FOR_{}", event_name);

    proc_macro::TokenStream::from(quote! {
        #[allow(non_upper_case_globals)]
        const #impl_wrapper_ident: () = {
            use oasis_std::{abi::*, exe::{encode_event_topic, Event}};

            impl#generics Event for #event_name#generics  {
                fn emit(&self) {
                    let topics: &[[u8; 32]] = &[
                        encode_event_topic(&stringify!(#event_name)),
                        #(encode_event_topic(&self.#indexed_field_idents)),*
                    ];
                    let topic_refs: Vec<&[u8]> = topics.iter().map(|t| t.as_ref()).collect();
                    oasis_std::backend::emit(&topic_refs, &self.try_to_vec().unwrap());
                }
            }
        };
    })
}
