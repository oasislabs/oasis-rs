macro_rules! emit {
    ($tok:expr, $fstr:literal$(,)? $( $arg:expr ),*) => {
        emit!(error, $tok, $fstr, $($arg),*)
    };
    ($level:ident, $tok:expr, $fstr:literal$(,)? $( $arg:expr ),*) => {
        $tok.span().unwrap().$level(format!($fstr, $($arg),*)).emit();
    };
}

macro_rules! check_next_arg {
    ($decl:ident, $inps:ident, $cond:expr, $err_msg:expr) => {
        let err_loc = match $inps.peek() {
            Some(inp) => inp.span(),
            None => $decl.inputs.span(),
        }
        .unwrap();
        if !$inps.next().map($cond).unwrap_or(false) {
            err_loc.error($err_msg).emit();
        }
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

fn keccak_key(ident: &syn::Ident) -> proc_macro2::TokenStream {
    let key = syn::parse_str::<syn::Expr>(&format!(
        "{:?}",
        tiny_keccak::keccak256(ident.to_string().as_bytes())
    ))
    .unwrap();
    quote! { H256::from(&#key) }
}
