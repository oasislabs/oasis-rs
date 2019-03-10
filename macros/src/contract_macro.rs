#[proc_macro]
pub fn contract(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let contract_def = parse_macro_input!(input as syn::File);
    let def_span = contract_def.span().unwrap();

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

    if contracts.len() == 0 {
        def_span
            .error("Contract definition must contain a #[derive(Contract)] struct.")
            .emit();
    } else if contracts.len() > 1 {
        emit_err!(
            contracts[1],
            "Contract definition must contain exactly one #[derive(Contract)] struct. Second occurrence here:"
        );
    }
    let contract = contracts.into_iter().nth(0).unwrap();
    let contract_name = &contract.ident;

    // TODO: generate RPC enum, dispatch tree
    let rpc_methods: Vec<proc_macro2::TokenStream> = other_items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(imp) if is_impl_of(&imp, contract_name) => Some(imp),
            _ => None,
        })
        .map(|imp| {
            let rpc_methods: Vec<proc_macro2::TokenStream> = imp
                .items
                .iter()
                .filter_map(|item| match item {
                    syn::ImplItem::Method(
                        m @ syn::ImplItemMethod {
                            vis: syn::Visibility::Public(_),
                            ..
                        },
                    ) => Some(m),
                    _ => None,
                })
                .inspect(|m| check_rpc_call(&imp, m))
                .map(|method| {
                    let rpc_name = format!("_call_{}", method.sig.ident);
                    // println!("{:#?}", method.sig.decl);
                    quote! {
                        fn #rpc_name(metho) {
                        }
                    }
                })
                .collect();

            let typ = &*imp.self_ty;
            quote! {
                impl #typ {
                    // fn _call_#
                }
            }
        })
        .collect();

    let deploy_mod_ident = format_ident!("_deploy_{}", contract_name);
    proc_macro::TokenStream::from(quote! {
        use oasis_std::prelude::*;

        #contract

        #(#other_items)*

        #[cfg(feature = "deploy")]
        mod #deploy_mod_ident {
            use super::*;

            #[derive(serde::Serialize, serde::Deserialize)]
            #[serde(tag = "method", content = "payload")]
            enum RPC {
                get_links(),
                add_link {
                    olink: String,
                    url: String,
                },
            }

            #[no_mangle]
            fn call() {
                let mut contract = <#contract_name>::coalesce();
                let payload: RPC = serde_cbor::from_slice(&oasis::input()).unwrap();
                let result = match payload {
                    RPC::get_links() => {
                        serde_cbor::to_vec(&contract.get_links())
                    }
                    RPC::add_link { olink, url } => {
                        serde_cbor::to_vec(&contract.add_link(olink, url))
                    }
                }.unwrap();
                OLinks::sunder(contract);
                oasis::ret(result);
            }
        }
    })
}
