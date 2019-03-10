#[proc_macro_derive(Contract)]
pub fn contract_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let contract = input.ident;
    proc_macro::TokenStream::from(quote! {
        impl Contract for #contract {
            fn coalesce() -> Self {
                // TODO: pull each non-lazy field out of storage and deserialize
                unimplemented!();
            }

            fn sunder(c: Self) {
                // TODO: serlalize each populated field to storage keys
                unimplemented!();
            }
        }
    })
}
