macro_rules! err {
    ($( $tok:ident ).+ : $fstr:literal$(,)? $( $arg:expr ),*) => {
        err!([error] $($tok).+ : $fstr, $($arg),*)
    };
    ([ $level:ident ] $( $tok:ident ).+ : $fstr:literal$(,)? $( $arg:expr ),*) => {
        $($tok).+.span().unwrap().$level(format!($fstr, $($arg),*)).emit();
    };
}

/// Checks whether struct derives a trait.
/// Currently fails if trait is a path instead of an ident (@see syn#597)
fn has_derive(s: &syn::ItemStruct, derive: &str) -> bool {
    s.attrs.iter().any(|attr| match attr.parse_meta() {
        Ok(syn::Meta::List(l)) => {
            l.ident == "derive"
                && l.nested.iter().any(|nest| match nest {
                    syn::NestedMeta::Meta(m) => &m.name() == derive,
                    _ => false,
                })
        }
        _ => false,
    })
}

/// Checks if `impl T` is for a given ident `U`
fn is_impl_of(imp: &syn::ItemImpl, typ: &syn::Ident) -> bool {
    match &*imp.self_ty {
        syn::Type::Path(tp) if &tp.path.segments.last().unwrap().value().ident == typ => true,
        _ => false,
    }
}

/// Hashes an ident into a `[u8; 32]` `TokenStream`.
fn keccak_key(ident: &syn::Ident) -> proc_macro2::TokenStream {
    let key = syn::parse_str::<syn::Expr>(&format!(
        "{:?}",
        tiny_keccak::keccak256(ident.to_string().as_bytes())
    ))
    .unwrap();
    quote! { H256::from(&#key) }
}

/// Turns `ctx: &Context` into `_: &Context` to prevent unused argument warnings.
fn mark_ctx_unused(sig: &mut syn::MethodSig) {
    sig.decl.inputs.iter_mut().for_each(|inp| {
        if let syn::FnArg::Captured(syn::ArgCaptured { ty, pat, .. }) = inp {
            if ty == &parse_quote!(&Context) || ty == &parse_quote!(&oasis_std::Conetxt) {
                *pat = parse_quote!(_);
            }
        }
    });
}

struct Deborrower {}
impl syn::visit_mut::VisitMut for Deborrower {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        if let syn::Type::Reference(syn::TypeReference { box elem, .. }) = ty {
            match elem {
                syn::Type::Path(syn::TypePath { path, .. }) if path.is_ident("str") => {
                    *ty = parse_quote!(String);
                }
                _ => {
                    *ty = elem.clone();
                }
            }
        }
        syn::visit_mut::visit_type_mut(self, ty);
    }
}
