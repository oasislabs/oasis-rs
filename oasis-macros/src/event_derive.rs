#[proc_macro_derive(Event, attributes(indexed))]
pub fn event_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let event_name = &input.ident;

    proc_macro::TokenStream::from(quote! {
        impl oasis_std::exe::Event for #event_name  {
            fn emit(&self) {
                unimplemented!()
            }
        }
    })
}
