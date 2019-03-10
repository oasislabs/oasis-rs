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

struct LazyInserter {}

impl syn::visit_mut::VisitMut for LazyInserter {
    fn visit_field_value_mut(&mut self, fv: &mut syn::FieldValue) {
        match fv.expr {
            syn::Expr::Macro(ref m) => {
                if m.mac
                    .path
                    .segments
                    .last()
                    .map(|punct| punct.value().ident == parse_quote!(lazy): syn::Ident)
                    .unwrap_or(false)
                {
                    let field = &fv.member;
                    let field = format!("{}", quote!( #field ));
                    let key = quote! { tiny_keccak::keccak256(#field.as_bytes()) };
                    let val = &m.mac.tts;
                    fv.expr = parse_quote!(Lazy::new(H256::from(#key), #val));
                }
            }
            _ => (),
        }
        syn::visit_mut::visit_field_value_mut(self, fv);
    }
}

struct RPC<'a> {
    ident: &'a syn::Ident,
    inputs: Vec<(&'a syn::Pat, &'a syn::Type)>,
}

impl<'a> RPC<'a> {
    fn new(imp: &'a syn::ItemImpl, m: &'a syn::ImplItemMethod) -> Self {
        let sig = &m.sig;
        let ident = &sig.ident;
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

        let typ = &*imp.self_ty;
        let mut inps = decl.inputs.iter().peekable();
        if ident == &parse_quote!(new): &syn::Ident {
            check_next_arg!(
                decl,
                inps,
                RPC::is_context,
                format!(
                    "`{}::new` must take `Context` as its first argument",
                    quote!(#typ)
                )
            );
            match &decl.output {
                syn::ReturnType::Type(_, t) if &**t == typ || t == &parse_quote!(Self) => (),
                ret => {
                    emit_err!(ret, format!("`{}::new` must return `Self`", quote!(#typ)));
                }
            }
            Self {
                ident,
                inputs: inps.filter_map(RPC::check_arg).collect(),
            }
        } else {
            check_next_arg!(
                decl,
                inps,
                RPC::is_self_ref,
                format!(
                    "First argument to `{}::{}` should be &[mut ]self.",
                    quote!(#typ),
                    quote!(ident)
                )
            );
            check_next_arg!(
                decl,
                inps,
                RPC::is_context,
                format!(
                    "Second argument to `{}::{}` should be &Context.",
                    quote!(#typ),
                    quote!(ident)
                )
            );
            Self {
                ident,
                inputs: inps.filter_map(RPC::check_arg).collect(),
            }
        }
    }

    fn is_context(arg: &syn::FnArg) -> bool {
        match arg {
            syn::FnArg::Captured(syn::ArgCaptured { ty, .. })
                if ty == &parse_quote!(Context) || ty == &parse_quote!(oasis_std::Context) =>
            {
                true
            }
            _ => false,
        }
    }

    fn is_self_ref(arg: &syn::FnArg) -> bool {
        match arg {
            syn::FnArg::SelfRef(_) => true,
            _ => false,
        }
    }

    fn check_arg(arg: &syn::FnArg) -> Option<(&syn::Pat, &syn::Type)> {
        match arg {
            syn::FnArg::Captured(syn::ArgCaptured { pat, ty, .. }) => Some((pat, ty)),
            syn::FnArg::Ignored(_) => {
                emit_err!(arg, "Arguments to RPCs must have explicit names.");
                None
            }
            syn::FnArg::Inferred(_) => {
                emit_err!(arg, "Arguments to RPCs must have explicit types.");
                None
            }
            _ => None,
        }
    }

    fn structify_inps(&self) -> Vec<proc_macro2::TokenStream> {
        self.inputs
            .iter()
            .map(|(name, ty)| {
                quote! { #name: #ty }
            })
            .collect()
    }

    fn input_names(&self) -> Vec<proc_macro2::TokenStream> {
        self.inputs
            .iter()
            .map(|(name, _ty)| quote!( #name ))
            .collect()
    }
}
