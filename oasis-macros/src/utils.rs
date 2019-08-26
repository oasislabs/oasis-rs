macro_rules! err {
    ($( $tok:ident ).+ : $fstr:literal$(,)? $( $arg:expr ),*) => {
        err!([error] $($tok).+ : $fstr, $($arg),*)
    };
    ([ $level:ident ] $( $tok:ident ).+ : $fstr:literal$(,)? $( $arg:expr ),*) => {
        $($tok).+.span().unwrap().$level(format!($fstr, $($arg),*)).emit();
    };
}

/// Hashes an ident into a `[u8; 32]` `TokenStream`.
fn keccak_key(ident: &syn::Ident) -> proc_macro2::TokenStream {
    let key = syn::parse_str::<syn::Expr>(&format!(
        "{:?}",
        tiny_keccak::keccak256(ident.to_string().as_bytes())
    ))
    .unwrap();
    quote! { #key }
}
