#[proc_macro_derive(Event, attributes(indexed))]
pub fn event_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let event_name = &input.ident;

    let indexed_fields = match input.data {
        syn::Data::Struct(syn::DataStruct { fields, .. }) => fields
            .iter()
            .filter(|f| f.attrs.iter().any(|attr| attr.path.is_ident("indexed")))
            .cloned()
            .collect::<Vec<_>>(),
        _ => {
            err!(input: "An `Event` must be a non-tuple struct.");
            return proc_macro::TokenStream::new();
        }
    };

    let impl_wrapper_ident = format_ident!("_IMPL_EVENT_FOR_{}", event_name);
    let topics_struct_ident = format_ident!("{}Topics", event_name);

    let mut option_fields = Vec::with_capacity(indexed_fields.len());
    let mut topic_setters = Vec::with_capacity(indexed_fields.len());
    let mut topic_getters = Vec::with_capacity(indexed_fields.len());
    let mut topic_assigners = Vec::with_capacity(indexed_fields.len());
    for f in indexed_fields.iter() {
        let ident = f.ident.as_ref().expect("Event must have named fields");
        let setter_ident = format_ident!("set_{}", ident);
        let ty = &f.ty;

        option_fields.push(quote! { #ident: Option<[u8; 32]> });
        topic_setters.push(quote! {
            fn #setter_ident(&mut self, #ident: &#ty) -> &mut Self {
                self.#ident = Some(tiny_keccak::keccak256(&serde_cbor::to_vec(#ident).unwrap()));
                self
            }
        });
        topic_getters.push(quote! { self.#ident.unwrap_or_default() });
        topic_assigners.push(quote! { .#setter_ident(&self.#ident) });
    }

    let event_name_topic = keccak_key(&event_name);

    proc_macro::TokenStream::from(quote! {
        mod #impl_wrapper_ident {
            use super::*;
            use mantle::reexports::*;

            #[derive(Default)]
            pub struct #topics_struct_ident {
                #(#option_fields),*
            }

            impl #topics_struct_ident {
                #(#topic_setters)*

                pub fn hash(&mut self) -> Vec<[u8; 32]> {
                    vec![ #(#topic_getters),* ]
                }
            }

            impl mantle::exe::Event for #event_name  {
                type Topics = #topics_struct_ident;

                fn emit(&self) {
                    let indexed_topics = Self::Topics::default()
                        #(#topic_assigners)*
                        .hash();
                    let topics = std::iter::once(#event_name_topic)
                        .chain(indexed_topics.into_iter())
                        .collect::<Vec<_>>();
                    mantle::ext::log(&topics, &serde_cbor::to_vec(self).unwrap());
                }
            }
        }
    })
}
