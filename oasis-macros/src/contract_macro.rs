#[proc_macro]
pub fn contract(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let contract_def = parse_macro_input!(input as syn::File);

    let mut contract: Option<syn::ItemStruct> = None;
    let mut other_items: Vec<syn::Item> = Vec::new();
    for item in contract_def.items.into_iter() {
        match item {
            syn::Item::Struct(s) if has_derive(&s, "Contract") => {
                if contract.is_none() {
                    contract.replace(s);
                } else {
                    err!(s: "`contract!` must contain exactly one #[derive(Contract)] struct. Additional occurrence here:");
                    other_items.push(s.into());
                }
            }
            _ => other_items.push(item),
        };
    }

    let preamble = quote! {
        #[macro_use]
        extern crate oasis_std;

        #[macro_use]
        extern crate serde;

        use oasis_std::prelude::*;
    };

    let contract = match contract {
        Some(contract) => contract,
        None => {
            proc_macro::Span::call_site()
                .error("Contract definition must contain a #[derive(Contract)] struct.")
                .emit();
            return proc_macro::TokenStream::from(quote! {
                #preamble

                #(#other_items)*
            });
        }
    };
    let contract_name = &contract.ident;

    // Transform `lazy!(val)` into `Lazy::_new(key, val)`.
    other_items.iter_mut().for_each(|item| {
        LazyInserter {}.visit_item_mut(item);
    });

    let impls: Vec<&syn::ItemImpl> = other_items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(imp) if is_impl_of(&imp, contract_name) => Some(imp),
            _ => None,
        })
        .collect();

    let (ctor, rpcs): (Vec<RPC>, Vec<RPC>) = impls
        .iter()
        .flat_map(|imp| {
            imp.items.iter().filter_map(move |item| match item {
                syn::ImplItem::Method(m) => match m.vis {
                    syn::Visibility::Public(_) => Some(RPC::new(imp, m)),
                    _ if m.sig.ident == "new" => {
                        err!(m: "`{}::new` should have `pub` visibility", contract_name);
                        Some(RPC::new(imp, m))
                    }
                    _ => None,
                },
                _ => None,
            })
        })
        .partition(|rpc| rpc.ident == "new");

    let empty_new = syn::Ident::new("new", proc_macro2::Span::call_site());
    let ctor = ctor.into_iter().nth(0).unwrap_or_else(|| {
        err!(contract_name: "Missing implementation for `{}::new`.", contract_name);
        RPC {
            ident: &empty_new,
            inputs: Vec::new(),
        }
    });

    let rpc_defs: Vec<proc_macro2::TokenStream> = rpcs
        .iter()
        .map(|rpc| {
            let ident = rpc.ident;
            let inps = rpc.structify_inps();
            // e.g., `my_method { my_input: String, my_other_input: u64 }`
            quote! {
                #ident { #(#inps),* }
            }
        })
        .collect();

    // Generate match arms to statically dispatch RPCs based on deserialized payload.
    let call_tree: Vec<proc_macro2::TokenStream> = rpcs
        .iter()
        .map(|rpc| {
            let ident = rpc.ident;
            let arg_names = rpc.input_names();
            let call_names = arg_names.clone();
            quote! {
                RPC::#ident { #(#arg_names),* } => {
                    serde_cbor::to_vec(&contract.#ident(Context {}, #(#call_names),*))
                }
            }
        })
        .collect();

    let (ctor_inps, ctor_args) = (ctor.structify_inps(), ctor.input_names());

    let deploy_payload = if ctor_inps.is_empty() {
        quote! {}
    } else {
        quote! { let payload: Ctor = serde_cbor::from_slice(&oasis::input()).unwrap(); }
    };

    let deploy_mod_ident =
        syn::Ident::new(&format!("_deploy_{}", contract_name), contract_name.span());
    proc_macro::TokenStream::from(quote! {
        #preamble

        #contract

        #(#other_items)*

        #[cfg(feature = "deploy")]
        #[allow(non_snake_case)]
        mod #deploy_mod_ident {
            use super::*;

            #[derive(serde::Serialize, serde::Deserialize)]
            #[serde(tag = "method", content = "payload")]
            #[allow(non_camel_case_types)]
            enum RPC {
                #(#rpc_defs),*
            }

            #[no_mangle]
            fn call() {
                let mut contract = <#contract_name>::coalesce();
                let payload: RPC = serde_cbor::from_slice(&oasis::input()).unwrap();
                let result = match payload {
                    #(#call_tree),*
                }.unwrap();
                OLinks::sunder(contract);
                oasis::ret(&result);
            }

            struct Ctor {
                #(#ctor_inps),*
            }

            #[no_mangle]
            pub fn deploy() {
                #deploy_payload
                #contract_name::sunder(#contract_name::new(Context {}, #(payload.#ctor_args),*));
            }
        }
    })
}

struct LazyInserter {}
impl syn::visit_mut::VisitMut for LazyInserter {
    fn visit_field_value_mut(&mut self, fv: &mut syn::FieldValue) {
        match fv.expr {
            syn::Expr::Macro(ref m) if m.mac.path.is_ident("lazy") => {
                let key = match fv.member {
                    syn::Member::Named(ref ident) => keccak_key(ident),
                    syn::Member::Unnamed(syn::Index { index, .. }) => {
                        quote! { H256::from(#index as u32) }
                    }
                };
                let val = &m.mac.tts;
                fv.expr = parse_quote!(Lazy::_new(H256::from(#key), #val));
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
                Self::is_context,
                "`{}::new` must take `Context` as its first argument",
                typ
            );
            match &decl.output {
                syn::ReturnType::Type(_, t) if &**t == typ || t == &parse_quote!(Self) => (),
                ret => {
                    err!(ret: "`{}::new` must return `Self`", quote!(#typ));
                }
            }
            Self {
                ident,
                inputs: inps.filter_map(RPC::check_arg).collect(),
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
                Self::is_context,
                "Second argument to `{}::{}` should be `Context`.",
                typ,
                ident
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
        self.inputs
            .iter()
            .map(|(name, ty)| quote!( #name: #ty ))
            .collect()
    }

    fn input_names(&self) -> Vec<proc_macro2::TokenStream> {
        self.inputs
            .iter()
            .map(|(name, _ty)| quote!( #name ))
            .collect()
    }
}
