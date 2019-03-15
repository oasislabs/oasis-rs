struct RPC<'a> {
    pub sig: &'a syn::MethodSig,
    pub inputs: Vec<(&'a syn::Pat, &'a syn::Type)>,
}

impl<'a> RPC<'a> {
    fn new(imp: &'a syn::ItemImpl, m: &'a syn::ImplItemMethod) -> Self {
        let sig = &m.sig;
        let ident = &sig.ident;
        if let Some(abi) = &sig.abi {
            err!(abi: "RPC methods cannot declare an ABI.");
        }
        if let Some(unsafe_) = sig.unsafety {
            err!(unsafe_: "RPC methods may not be unsafe.");
        }
        let decl = &sig.decl;
        if decl.generics.type_params().count() > 0 {
            err!(
                decl.generics:
                "RPC methods may not have generic type parameters.",
            );
        }
        if let Some(variadic) = decl.variadic {
            err!(variadic: "RPC methods may not be variadic.");
        }

        let typ = &*imp.self_ty;
        let mut inps = decl.inputs.iter().peekable();
        if ident == "new" {
            check_next_arg!(
                sig,
                inps,
                Self::is_context_ref,
                "`{}::new` must take `&Context` as its first argument",
                typ
            );
            match &decl.output {
                syn::ReturnType::Type(_, box syn::Type::Path(syn::TypePath { path, .. }))
                    if path
                        .segments
                        .last()
                        .map(|seg| {
                            let seg = seg.value();
                            seg.ident == "Result"
                                && match &seg.arguments {
                                    syn::PathArguments::AngleBracketed(bracketed) => {
                                        let args = &bracketed.args;
                                        args.len() == 1 && {
                                            match args.first().unwrap().value() {
                                                syn::GenericArgument::Type(t) => {
                                                    t == &parse_quote!(Self) || t == typ
                                                }
                                                _ => false,
                                            }
                                        }
                                    }
                                    _ => false,
                                }
                        })
                        .unwrap_or(false) =>
                {
                    ()
                }
                ret => {
                    err!(ret: "`{}::new` must return `Result<Self>`", quote!(#typ));
                }
            }
        } else {
            check_next_arg!(
                sig,
                inps,
                Self::is_self_ref,
                "First argument to `{}::{}` should be `&self` or `&mut self`.",
                typ,
                ident
            );
            check_next_arg!(
                sig,
                inps,
                Self::is_context_ref,
                "Second argument to `{}::{}` should be `&Context`.",
                typ,
                ident
            );
        }
        Self {
            sig,
            inputs: inps.filter_map(RPC::check_arg).collect(),
        }
    }

    fn is_context_ref(arg: &syn::FnArg) -> bool {
        match arg {
            syn::FnArg::Captured(syn::ArgCaptured { ty, .. })
                if ty == &parse_quote!(&Context) || ty == &parse_quote!(&oasis_std::Context) =>
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
                err!(arg: "Arguments to RPCs must have explicit names.");
                None
            }
            syn::FnArg::Inferred(_) => {
                err!(arg: "Arguments to RPCs must have explicit types.");
                None
            }
            _ => None,
        }
    }

    fn structify_inps(&self) -> Vec<proc_macro2::TokenStream> {
        let string_type = parse_quote!(String);
        self.inputs
            .iter()
            .map(|(name, ty)| {
                let owned_ty = match ty {
                    syn::Type::Reference(syn::TypeReference { elem, .. }) => match &**elem {
                        syn::Type::Path(syn::TypePath { path, .. }) if path.is_ident("str") => {
                            &string_type
                        }
                        _ => &**elem,
                    },
                    _ => *ty,
                };
                quote!( #name: #owned_ty )
            })
            .collect()
    }

    fn input_names(&self) -> Vec<proc_macro2::TokenStream> {
        self.inputs
            .iter()
            .map(|(name, _ty)| quote!( #name ))
            .collect()
    }

    fn call_args(&self) -> Vec<proc_macro2::TokenStream> {
        self.inputs
            .iter()
            .map(|(name, ty)| match ty {
                syn::Type::Reference(_) => quote! { &#name },
                _ => quote! { #name },
            })
            .collect()
    }
}
