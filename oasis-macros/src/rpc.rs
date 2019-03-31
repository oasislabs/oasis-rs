struct RPC<'a> {
    pub sig: &'a syn::MethodSig,
    pub inputs: Vec<(&'a syn::Pat, &'a syn::Type)>,
}

impl<'a> RPC<'a> {
    fn new(self_ty: &'a syn::Type, m: &'a syn::ImplItemMethod) -> Result<Self, Self> {
        let sig = &m.sig;
        let ident = &sig.ident;
        let mut has_err = false;
        if let Some(abi) = &sig.abi {
            err!(abi: "RPC methods cannot declare an ABI.");
            has_err = true;
        }
        if let Some(unsafe_) = sig.unsafety {
            err!(unsafe_: "RPC methods may not be unsafe.");
            has_err = true;
        }
        let decl = &sig.decl;
        if decl.generics.type_params().count() > 0 {
            err!(
                decl.generics:
                "RPC methods may not have generic type parameters.",
            );
            has_err = true;
        }
        if let Some(variadic) = decl.variadic {
            err!(variadic: "RPC methods may not be variadic.");
            has_err = true;
        }

        let mut inps = decl.inputs.iter().peekable();

        macro_rules! check_next_arg {
            ($cond:expr, $err_msg:expr, $( $arg:ident ),*) => {
                let err_loc = match inps.peek() {
                    Some(inp) => inp.span(),
                    None => sig.ident.span(),
                }
                .unwrap();
                if !inps.next().map($cond).unwrap_or(false) {
                    err_loc.error(format!($err_msg, $( quote!(#$arg) ),*)).emit();
                    has_err = true;
                }
            };
        }

        if ident == "new" {
            check_next_arg!(
                Self::is_context_ref,
                "`{}::new` must take `&Context` as its first argument",
                self_ty
            );
            if let None = Self::unpack_output(&decl.output) {
                err!(decl.output: "`{}::new` must return `Result<Self>`", quote!(#self_ty));
            }
        } else {
            check_next_arg!(
                Self::is_self_ref,
                "First argument to `{}::{}` should be `&self` or `&mut self`.",
                self_ty,
                ident
            );
            check_next_arg!(
                Self::is_context_ref,
                "Second argument to `{}::{}` should be `&Context`.",
                self_ty,
                ident
            );
            if Self::unpack_output(&decl.output).is_none() {
                err!(decl.output: "`{}::new` must return `Result<T>`", quote!(#self_ty));
                has_err = true;
            }
        }
        let rpc = Self {
            sig,
            inputs: inps.filter_map(RPC::check_arg).collect(),
        };
        if !has_err {
            Ok(rpc)
        } else {
            Err(rpc)
        }
    }

    /// Checks if an arg is `&[oasis_std::]Context`
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

    /// Checks if an arg is `&self` or `&mut self`.
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

    /// Returns `field: type` statements as would be present in a `struct` item
    /// corresponding to the owned types of the RPC args.
    fn structify_inps(&self) -> Vec<proc_macro2::TokenStream> {
        self.inputs
            .iter()
            .map(|(name, ty)| {
                let mut owned_ty = (*ty).clone();
                Deborrower {}.visit_type_mut(&mut owned_ty);
                quote!( #name: #owned_ty )
            })
            .collect()
    }

    /// Returns the idents of the RpcPayload inputs.
    fn input_names(&self) -> Vec<proc_macro2::TokenStream> {
        self.inputs
            .iter()
            .map(|(name, _ty)| quote!( #name ))
            .collect()
    }

    /// Turns owned RpcPayload input idents into (possibly borrowed) call arg exprs.
    fn call_args(&self) -> Vec<proc_macro2::TokenStream> {
        self.inputs
            .iter()
            .map(|(name, ty)| match ty {
                syn::Type::Reference(_) => quote! { &#name },
                _ => quote! { #name },
            })
            .collect()
    }

    /// Extracts the `T` from `Result<T>`, if it exists.
    fn unpack_output(output: &syn::ReturnType) -> Option<&syn::Type> {
        match output {
            syn::ReturnType::Type(_, box syn::Type::Path(syn::TypePath { path, .. })) => path
                .segments
                .last()
                .map(|seg| {
                    let seg = seg.value();
                    if seg.ident != "Result" {
                        return None;
                    }
                    match &seg.arguments {
                        syn::PathArguments::AngleBracketed(bracketed) => {
                            let args = &bracketed.args;
                            if args.len() != 1 {
                                return None;
                            }
                            match args.first().unwrap().value() {
                                syn::GenericArgument::Type(t) => Some(t),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                })
                .unwrap_or(None),
            _ => None,
        }
    }

    fn result_ty(&self) -> &syn::Type {
        Self::unpack_output(&self.sig.decl.output).expect("`Result` output checked in `new`")
    }
}
