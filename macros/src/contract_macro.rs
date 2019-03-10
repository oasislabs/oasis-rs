#[proc_macro]
pub fn contract(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let contract_def = parse_macro_input!(input as syn::File);
    let def_span = contract_def.span().unwrap(); // save this for error reporting later

    let mut contracts: Vec<syn::ItemStruct> = Vec::new();
    let mut other_items: Vec<syn::Item> = Vec::new();
    for item in contract_def.items.into_iter() {
        match item {
            syn::Item::Struct(s) if has_derive(&s, &parse_quote!(Contract)) => {
                contracts.push(s);
            }
            _ => other_items.push(item),
        };
    }

    if contracts.is_empty() {
        def_span
            .error("Contract definition must contain a #[derive(Contract)] struct.")
            .emit();
    } else if contracts.len() > 1 {
        emit_err!(
            contracts[1],
            "Contract definition must contain exactly one #[derive(Contract)] struct. Second occurrence here:"
        );
    }
    let contract = match contracts.into_iter().nth(0) {
        Some(contract) => contract,
        None => {
            return proc_macro::TokenStream::from(quote! {
                #(#other_items)*
            });
        }
    };
    let contract_name = &contract.ident;

    // transform `lazy!(val)` into `Lazy::_new(key, val)`
    other_items.iter_mut().for_each(|item| {
        LazyInserter {}.visit_item_mut(item);
    });

    let (ctor, rpcs): (Vec<RPC>, Vec<RPC>) = other_items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(imp) if is_impl_of(&imp, contract_name) => Some(imp),
            _ => None,
        })
        .flat_map(|imp| {
            imp.items.iter().filter_map(move |item| match item {
                syn::ImplItem::Method(
                    m @ syn::ImplItemMethod {
                        vis: syn::Visibility::Public(_),
                        ..
                    },
                ) => Some(RPC::new(imp, m)),
                _ => None,
            })
        })
        .partition(|rpc| rpc.ident == &parse_quote!(new): &syn::Ident);

    let ctor = ctor.into_iter().nth(0);

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

    // generate match arms to statically dispatch RPCs based on deserialized payload
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

    let (ctor_inps, ctor_args) = ctor
        .map(|ctor| (ctor.structify_inps(), ctor.input_names()))
        .unwrap_or((Vec::new(), Vec::new()));
    let deploy_payload = if ctor_inps.is_empty() {
        quote! {}
    } else {
        quote! {
            let payload: Ctor = serde_cbor::from_slice(&oasis::input()).unwrap();
        }
    };

    let deploy_mod_ident = format_ident!("_deploy_{}", contract_name);
    proc_macro::TokenStream::from(quote! {
        #[macro_use]
        extern crate oasis_std;

        use oasis_std::prelude::*;

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
