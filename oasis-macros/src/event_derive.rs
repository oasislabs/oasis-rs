#[proc_macro_derive(Event, attributes(indexed))]
pub fn event_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let event_name = &input.ident;
    let generics = &input.generics;

    let indexed_fields = match input.data {
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(syn::FieldsNamed { named, .. }),
            ..
        }) => named
            .iter()
            .filter(|f| f.attrs.iter().any(|attr| attr.path.is_ident("indexed")))
            .cloned()
            .collect::<Vec<_>>(),
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Unit,
            ..
        }) => Vec::new(),
        _ => {
            err!(input: "An `Event` must be a non-tuple struct.");
            return proc_macro::TokenStream::new();
        }
    };

    let impl_wrapper_ident = format_ident!("_IMPL_EVENT_FOR_{}", event_name);
    let indexed_field_idents = indexed_fields.iter().map(|f| f.ident.as_ref().unwrap());
    let event_name_topic = keccak_key(&event_name);
    let num_topics = indexed_fields.len() + 1;

    proc_macro::TokenStream::from(quote! {
        #[allow(warnings)]
        const #impl_wrapper_ident: () = {
            use oasis_std::reexports::borsh::BorshSerialize as _;

            impl#generics oasis_std::exe::Event for #event_name#generics  {
                fn emit(&self) {
                    let hashes: Vec<[u8; 32]> = vec![
                        #(tiny_keccak::keccak256(&self.#indexed_field_idents.try_to_vec().unwrap())),*
                    ];
                    let mut topics: Vec<&[u8]> = Vec::with_capacity(#num_topics);
                    topics.push(&#event_name_topic);
                    topics.append(&mut hashes.iter().map(<_>::as_ref).collect());
                    oasis_std::backend::emit(&topics, &self.try_to_vec().unwrap());
                }
            }
        };
    })
}
