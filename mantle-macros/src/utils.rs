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
    quote! { mantle::types::H256::from(&#key) }
}

/// Recursively removes borrows from a type.  E.g., `&Vec<&str>` becomes `Vec<String>`.
struct Deborrower {}
impl syn::visit_mut::VisitMut for Deborrower {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        if let syn::Type::Reference(syn::TypeReference { box elem, .. }) = ty {
            match elem {
                syn::Type::Path(syn::TypePath { path, .. }) if path.is_ident("str") => {
                    *ty = parse_quote!(String);
                }
                syn::Type::Slice(syn::TypeSlice { box elem, .. }) => *ty = parse_quote!(Vec<#elem>),
                _ => {
                    *ty = elem.clone();
                }
            }
        }
        syn::visit_mut::visit_type_mut(self, ty);
    }
}

macro_rules! format_ident {
    ($fmt_str:literal, $($fmt_arg:expr),+) => {
        syn::Ident::new(&format!($fmt_str, $($fmt_arg),+), proc_macro2::Span::call_site())
    }
}

/// Removes a `r#` from an `Ident` if it exists. @see dtolnay/syn#603
fn unraw(ident: &syn::Ident) -> syn::Ident {
    let ident_str = ident.to_string();
    if ident_str.starts_with("r#") {
        format_ident!("{}", &ident_str[2..])
    } else {
        ident.clone()
    }
}
