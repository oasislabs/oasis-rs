include!("rpc.rs");

#[proc_macro_attribute]
pub fn contract(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let contract_def = parse_macro_input!(input as syn::ItemMod);

    let mut contract = None;
    let mut impls = Vec::new();
    let mut other_items = Vec::new();
    for item in contract_def.content.unwrap().1.into_iter() {
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
    let mut test_contract = contract.clone();
    PubCraterr {}.visit_item_struct_mut(&mut test_contract);
    let contract_ident = &contract.ident;

    if contract.generics.type_params().count() > 0 {
        err!(contract.generics: "Contract cannot contain generic types.");
        early_return!();
    }

    let mut contract_impls = Vec::new();
    for imp in impls.into_iter() {
        if is_impl_of(&imp, contract_ident) {
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
                        let rpc = match RPC::new(&*imp.self_ty, m) {
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
                            err!(m: "`{}::new` should have `pub` visibility", contract_ident);
                            early_return!();
                        }
                    }
                }
            }
        }
    }

    let empty_new: syn::ImplItemMethod = parse_quote!(
        pub fn new(ctx: &Context) -> Result<Self> {
            unreachable!()
        }
    );
    let ctor = ctor.into_iter().nth(0).unwrap_or_else(|| {
        err!(contract_ident: "Missing implementation for `{}::new`.", contract_ident);
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
                    let result = contract.#ident(&Context::default(), #(#call_args),*);
                    // TODO better error handling
                    serde_cbor::to_vec(&result.map_err(|err| err.to_string()))
                }
            }
        })
        .collect();

    let entry_fn_body = if rpcs.is_empty() {
        quote! {}
    } else {
        quote! {
            let mut contract = <#contract_ident>::coalesce();
            let payload: RpcPayload = serde_cbor::from_slice(&oasis::input()).unwrap();
            let result = match payload {
                #(#call_tree),*
            }.unwrap();
            #contract_ident::sunder(contract);
            oasis::ret(&result);
        }
    };

    let ctor_sig = ctor.sig;
    let ctor_ctx_ident = ctor.ctx_ident();
    let (ctor_inps, ctor_args) = (ctor.structify_inps(), ctor.call_args());
    let ctor_payload_inps: Vec<proc_macro2::TokenStream> = ctor_args
        .iter()
        .map(|arg| quote! { #arg: #arg.to_owned() })
        .collect();
    let deploy_payload = if ctor_args.is_empty() {
        quote! {}
    } else {
        quote! {
            let CtorPayload { #(#ctor_args)* } = serde_cbor::from_slice(&oasis::input()).unwrap();
        }
    };

    let client_impls: Vec<proc_macro2::TokenStream> = rpcs
        .iter()
        .map(|rpc| {
            let ctx_ident = rpc.ctx_ident();
            let mut sig = rpc.sig.clone();
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
                let result = oasis_std::testing::call_with(
                    &self._address,
                    &#ctx_ident,
                    &input,
                    &|| {
                        oasis::call(
                            #ctx_ident.gas_left(),
                            &self._address /* callee = address held by `Client` struct */,
                            #ctx_ident.value(),
                            &input
                        )
                    }
                )?;
                if cfg!(test) {
                    unsafe { &mut *self.contract.get() }.replace(TheContract::coalesce());
                }
                type RpcResult = std::result::Result<#result_ty, String>;
                // TODO: better error handling
                serde_cbor::from_slice::<RpcResult>(&result)?
                    .map_err(|err| failure::format_err!("{}", err))
            };

            quote! {
                pub #sig {
                    #rpc_inner
                }
            }
        })
        .collect();

    let client_ident = format_ident!("{}Client", contract.ident);

    proc_macro::TokenStream::from(quote! {
        #preamble

        #(#other_items)*

        #[allow(non_snake_case)]
        mod contract {
            use super::*;

            #[derive(serde::Serialize, serde::Deserialize)]
            struct CtorPayload {
                #(#ctor_inps),*
            }

            #[derive(serde::Serialize, serde::Deserialize, Debug)]
            #[serde(tag = "method", content = "payload")]
            #[allow(non_camel_case_types)]
            pub enum RpcPayload {
                #(#rpc_defs),*
            }

            mod contract {
                use super::*;

                #[cfg(not(test))]
                #contract

                #[cfg(test)]
                #test_contract

                #[cfg(any(feature = "deploy", test))]
                #(#contract_impls)*

                #[cfg(any(feature = "deploy", test))]
                pub(super) mod deploy {
                    use super::*;

                    #[no_mangle]
                    pub fn call() {
                        #entry_fn_body
                    }

                    #[no_mangle]
                    pub fn deploy() {
                        #deploy_payload
                        #contract_ident::sunder(
                            #contract_ident::new(&Context::default(), #(#ctor_args),*).unwrap()
                        );
                    }
                }
            }

            #[cfg(any(not(feature = "deploy"), test))]
            mod client {
                use std::cell::UnsafeCell;

                use super::*;
                use contract::#contract_ident as TheContract;

                pub struct #client_ident {
                    contract: UnsafeCell<Option<TheContract>>, // `Some` during testing
                    _address: Address,
                }

                #[cfg(test)]
                impl std::ops::Deref for #client_ident {
                    type Target = TheContract;
                    fn deref(&self) -> &Self::Target {
                        unsafe { &mut *self.contract.get() }.as_ref().unwrap()
                    }
                }

                #[cfg(test)]
                impl std::ops::DerefMut for #client_ident {
                    fn deref_mut(&mut self) -> &mut Self::Target {
                        unsafe { &mut *self.contract.get() }.as_mut().unwrap()
                    }
                }

                impl #client_ident {
                    #(#client_impls)*

                    #[cfg(not(test))]
                    #[allow(unused_variables)]
                    pub #ctor_sig {
                        let contract_addr = oasis::create(
                            #ctor_ctx_ident.value.unwrap_or_default(),
                            include_bytes!(concat!(
                                env!("CARGO_MANIFEST_DIR"), "/target/contract/",
                                env!("CARGO_PKG_NAME"), ".wasm"
                            )),
                        )?;
                        Ok(Self {
                            contract: UnsafeCell::new(None),
                            _address: contract_addr,
                        })
                    }

                    #[cfg(test)]
                    pub #ctor_sig {
                        let contract_addr = oasis_std::testing::create_account(
                            #ctor_ctx_ident.value.unwrap_or_default()
                        );
                        let payload = CtorPayload { #(#ctor_payload_inps),* };
                        oasis_std::testing::call_with(
                            &contract_addr,
                            &#ctor_ctx_ident,
                            &serde_cbor::to_vec(&payload).unwrap(),
                            &contract::deploy::deploy
                        );
                        Ok(Self {
                            contract: UnsafeCell::new(
                                Some(TheContract::new(#ctor_ctx_ident, #(#ctor_args),*)?)
                            ),
                            _address: contract_addr,
                        })
                    }

                    pub fn at(address: Address) -> Self {
                        Self {
                            contract: UnsafeCell::new(None),
                            _address: address,
                        }
                    }

                    pub fn address(&self) -> Address {
                        self._address
                    }
                }
            }

            #[cfg(any(not(feature = "deploy"), test))]
            pub use client::#client_ident as #contract_ident;
        }
        #[cfg(any(not(feature = "deploy"), test))]
        pub use contract::#contract_ident;
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

/// Used to increase the visibility of contract struct fields to at least `pub(crate)`
/// so that testing client can proxy field access via `Deref`.
struct PubCraterr {}
impl syn::visit_mut::VisitMut for PubCraterr {
    fn visit_visibility_mut(&mut self, vis: &mut syn::Visibility) {
        match vis {
            syn::Visibility::Inherited | syn::Visibility::Restricted(_) => {
                *vis = parse_quote!(pub(crate));
            }
            _ => (),
        }
        syn::visit_mut::visit_visibility_mut(self, vis);
    }
}
