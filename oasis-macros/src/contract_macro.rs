include!("rpc.rs");

#[proc_macro]
pub fn contract(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let contract_def = parse_macro_input!(input as syn::File);

    let mut contract = None;
    let mut impls = Vec::new();
    let mut other_items = Vec::new();
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
            syn::Item::Impl(i) => impls.push(i),
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

    macro_rules! early_return {
        () => {
            return proc_macro::TokenStream::from(quote! {
                #preamble
                #(#other_items)*
            });
        };
    }

    let contract = match contract {
        Some(contract) => contract,
        None => {
            proc_macro::Span::call_site()
                .error("Contract definition must contain a #[derive(Contract)] struct.")
                .emit();
            early_return!();
        }
    };
    let contract_name = &contract.ident;

    if contract.generics.type_params().count() > 0 {
        err!(contract.generics: "Contract cannot contain generic types.");
        early_return!();
    }

    let mut contract_impls = Vec::new();
    for imp in impls.into_iter() {
        if is_impl_of(&imp, contract_name) {
            contract_impls.push(imp);
        } else {
            other_items.push(imp.into());
        }
    }

    // Transform `lazy!(val)` into `Lazy::_new(key, val)`.
    contract_impls.iter_mut().for_each(|item| {
        LazyInserter {}.visit_item_impl_mut(item);
    });

    let mut ctor = None;
    let mut rpcs = Vec::new();
    for imp in contract_impls.iter() {
        for item in imp.items.iter() {
            if let syn::ImplItem::Method(m) = item {
                match m.vis {
                    syn::Visibility::Public(_) => {
                        let rpc = match RPC::new(imp, m) {
                            Ok(rpc) => rpc,
                            Err(_) => early_return!(),
                        };
                        if m.sig.ident == "new" {
                            ctor.replace(rpc);
                        } else {
                            rpcs.push(rpc);
                        }
                    }
                    _ => {
                        if m.sig.ident == "new" {
                            err!(m: "`{}::new` should have `pub` visibility", contract_name);
                            early_return!();
                        }
                    }
                }
            }
        }
    }

    let empty_new: syn::ImplItemMethod = parse_quote!(
        pub fn new() -> Result<Self> {
            unreachable!()
        }
    );
    let ctor = ctor.into_iter().nth(0).unwrap_or_else(|| {
        err!(contract_name: "Missing implementation for `{}::new`.", contract_name);
        RPC {
            sig: &empty_new.sig,
            inputs: Vec::new(),
        }
    });

    let rpc_defs: Vec<proc_macro2::TokenStream> = rpcs
        .iter()
        .map(|rpc| {
            let ident = &rpc.sig.ident;
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
            let ident = &rpc.sig.ident;
            let arg_names = rpc.input_names();
            let call_args = rpc.call_args();
            quote! {
                RpcPayload::#ident { #(#arg_names),* } => {
                    let result = contract.#ident(&Context {}, #(#call_args),*);
                    // TODO better error handling
                    serde_cbor::to_vec(&result.map_err(|err| err.to_string()))
                }
            }
        })
        .collect();

    let mut ctor_sig = ctor.sig.clone();
    mark_ctx_unused(&mut ctor_sig);
    let (ctor_inps, ctor_args) = (ctor.structify_inps(), ctor.call_args());
    let ctor_payload_inps: Vec<proc_macro2::TokenStream> = ctor_args
        .iter()
        .map(|arg| {
            let arg = arg.clone();
            quote! {
                #arg: #arg.to_owned()
            }
        })
        .collect();
    let ctor_new_args = ctor_args.clone();
    let deploy_payload = if ctor_args.is_empty() {
        quote! {}
    } else {
        let args = ctor_args.clone();
        quote! { let CtorPayload { #(#args)* } = serde_cbor::from_slice(&oasis::input()).unwrap(); }
    };

    let client_impls: Vec<proc_macro2::TokenStream> = rpcs
        .iter()
        .map(|rpc| {
            let mut sig = rpc.sig.clone();
            mark_ctx_unused(&mut sig);
            Deborrower {}.visit_return_type_mut(&mut sig.decl.output);

            let mut test_sig = sig.clone();
            // take `&mut self` to allow updatating test client state
            test_sig.decl.inputs[0] = parse_quote!(&mut self);

            let ident = &sig.ident;
            let inps = rpc.input_names().into_iter().map(|name| {
                quote! {
                    #name: #name.to_owned()
                }
            });

            let mut result_ty = rpc.result_ty().clone();
            Deborrower {}.visit_type_mut(&mut result_ty);

            let rpc_inner = quote! {
                let payload = RpcPayload::#ident { #(#inps),* };
                let input = serde_cbor::to_vec(&payload).unwrap();
                // TODO: populate `call` fields with actual values
                let result = oasis::call(
                    42 /* gas */,
                    &Address::zero(),
                    U256::from(0) /* value */,
                    &input
                )?;
                if cfg!(any(test, feature = "test")) {
                    *self = TheContract::coalesce().into(); // copy new state into testing client
                }
                type RpcResult = std::result::Result<#result_ty, String>;
                // TODO: better error handling
                serde_cbor::from_slice::<RpcResult>(&result)?
                    .map_err(|err| failure::format_err!("{}", err))
            };
            let test_rpc_inner = rpc_inner.clone();

            quote! {
                #[cfg(not(any(test, feature="test")))]
                pub #sig {
                    #rpc_inner
                }

                #[cfg(any(test, feature="test"))]
                pub #test_sig {
                    #test_rpc_inner
                }
            }
        })
        .collect();

    let mut client = contract.clone();
    client.fields.iter_mut().for_each(|f| {
        f.vis = parse_quote!(pub(crate));
    });
    client.attrs = client
        .attrs
        .into_iter()
        .filter_map(|mut attr| {
            if !attr.path.is_ident("derive") {
                return Some(attr);
            }
            match attr.parse_meta() {
                Ok(syn::Meta::List(syn::MetaList { nested, .. })) => {
                    let derives: Vec<&syn::NestedMeta> = nested
                        .iter()
                        .filter(|d| d != &&parse_quote!(Contract))
                        .collect();
                    if derives.is_empty() {
                        None
                    } else {
                        attr.tts = quote! { (#(#derives)*) };
                        Some(attr)
                    }
                }
                _ => None,
            }
        })
        .collect();

    let wrapper_mod_ident = syn::Ident::new(&format!("_{}_", contract_name), contract_name.span());
    proc_macro::TokenStream::from(quote! {
        #preamble

        #(#other_items)*

        #[allow(non_snake_case)]
        mod #wrapper_mod_ident {
            use super::*;

            #[derive(serde::Serialize, serde::Deserialize)]
            struct CtorPayload {
                #(#ctor_inps),*
            }

            #[derive(serde::Serialize, serde::Deserialize)]
            #[serde(tag = "method", content = "payload")]
            #[allow(non_camel_case_types)]
            pub enum RpcPayload {
                #(#rpc_defs),*
            }

            mod contract {
                use super::*;

                #contract

                #(#contract_impls)*

                #[cfg(any(feature = "deploy", test))]
                pub(super) mod deploy {
                    use super::*;

                    #[no_mangle]
                    pub fn call() {
                        let mut contract = <#contract_name>::coalesce();
                        let payload: RpcPayload = serde_cbor::from_slice(&oasis::input()).unwrap();
                        let result = match payload {
                            #(#call_tree),*
                        }.unwrap();
                        #contract_name::sunder(contract);
                        oasis::ret(&result);
                    }

                    #[no_mangle]
                    pub fn deploy() {
                        #deploy_payload
                        #contract_name::sunder(
                            #contract_name::new(&Context {}, #(#ctor_args),*).unwrap()
                        );
                    }
                }
            }

            mod client {
                use super::*;
                use contract::#contract_name as TheContract;

                #client

                impl #contract_name {
                    #(#client_impls)*

                    #[cfg(test)]
                    pub #ctor_sig {
                        let payload = CtorPayload { #(#ctor_payload_inps),* };
                        oasis_test::set_input(serde_cbor::to_vec(&payload).unwrap());
                        contract::deploy::deploy();
                        Ok(TheContract::new(&Context {}, #(#ctor_new_args),*)?.into())
                    }
                }

                impl From<TheContract> for #contract_name {
                    fn from(contract: TheContract) -> Self {
                        unsafe { std::mem::transmute::<TheContract, #contract_name>(contract) }
                    }
                }
            }

            pub use client::#contract_name;
        }
        pub use #wrapper_mod_ident::#contract_name;
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
