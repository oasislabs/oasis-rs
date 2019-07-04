#[proc_macro_attribute]
pub fn default(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::ImplItemMethod);
    proc_macro::TokenStream::from(quote!(#input))
}
