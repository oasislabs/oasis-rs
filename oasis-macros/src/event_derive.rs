#[proc_macro_derive(Event)]
pub fn event_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let event_name = &input.ident;

    match input.data {
        syn::Data::Struct(_) => (),
        _ => {
            err!(input: "An `Event` must be a non-tuple struct.");
            return proc_macro::TokenStream::new();
        }
    };

    let event_name_hash = static_hash(&event_name);

    let impl_wrapper_ident = format_ident!("_IMPL_EVENT_FOR_{}", event_name);

    proc_macro::TokenStream::from(quote! {
        const #impl_wrapper_ident: () = {
            impl oasis_std::exe::Event for #event_name  {
                fn emit(&self) {
                    oasis_std::ext::log(
                        &vec![#event_name_hash],
                        &serde_cbor::to_vec(self).unwrap(),
                    );
                }
            }
        };
    })
}
