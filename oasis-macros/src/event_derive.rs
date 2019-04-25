#[proc_macro_derive(Event)]
pub fn event_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let event_name = &input.ident;
    proc_macro::TokenStream::from(quote!())
    // proc_macro::TokenStream::from(match get_serde(&input) {
    //     Ok((ser, de)) => {
    //         quote! {
    //             impl Contract for #contract {
    //                 fn coalesce() -> Self {
    //                     #de
    //                 }
    //
    //                 fn sunder(contract: Self) {
    //                     #ser
    //                 }
    //             }
    //         }
    //     }
    //     Err(_) => quote! {},
    // })
}
