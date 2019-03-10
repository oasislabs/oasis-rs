macro_rules! emit_err {
    ($tok:expr, $msg:expr $(,)?) => {
        $tok.span().unwrap().error($msg).emit();
    };
}

macro_rules! format_ident {
    ($fstr:literal, $ident:expr) => {
        syn::Ident::new(&format!($fstr, $ident), $ident.span())
    };
}

fn has_derive(s: &syn::ItemStruct, derive: &syn::Ident) -> bool {
    s.attrs.iter().any(|attr| match attr.parse_meta() {
        Ok(syn::Meta::List(l)) => {
            l.ident == parse_quote!(derive): syn::Ident
                && l.nested.iter().any(|nest| match nest {
                    syn::NestedMeta::Meta(m) => &m.name() == derive,
                    _ => false,
                })
        }
        _ => false,
    })
}

fn is_impl_of(imp: &syn::ItemImpl, typ: &syn::Ident) -> bool {
    match &*imp.self_ty {
        syn::Type::Path(tp) if &tp.path.segments.last().unwrap().value().ident == typ => true,
        _ => false,
    }
}

fn check_rpc_call(imp: &syn::ItemImpl, m: &syn::ImplItemMethod) {
    let sig = &m.sig;
    if let Some(abi) = &sig.abi {
        emit_err!(abi, "RPC methods cannot declare an ABI.");
    }
    if let Some(unsafe_) = sig.unsafety {
        emit_err!(unsafe_, "RPC methods may not be unsafe.");
    }
    let decl = &sig.decl;
    if decl.generics.type_params().count() > 0 {
        emit_err!(
            decl.generics,
            "RPC methods may not have generic type parameters.",
        );
    }
    if let Some(variadic) = decl.variadic {
        emit_err!(variadic, "RPC methods may not be variadic.");
    }
    if sig.ident.to_string() == "new" {
        match decl.inputs.first().as_ref().map(|p| p.value()) {
            Some(syn::FnArg::Captured(syn::ArgCaptured { ty, .. }))
                if ty == &parse_quote!(Context) || ty == &parse_quote!(oasis_std::Context) =>
            {
                ()
            }
            inp => {
                emit_err!(
                    inp,
                    "RPC `new` must take `[oasis_std::]Context` as its first argument"
                );
            }
        };
        let typ = &*imp.self_ty;
        match &decl.output {
            syn::ReturnType::Type(_, t) if &**t == typ || t == &parse_quote!(Self) => (),
            ret => {
                emit_err!(ret, format!("`{}::new` must return `Self`", quote!(#typ)));
            }
        }
    }
}
