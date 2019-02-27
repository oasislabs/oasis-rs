//! Ethereum (Solidity) derivation for rust contracts (compiled to wasm or otherwise)

#![recursion_limit = "128"]
#![deny(unused)]

extern crate proc_macro;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate syn;
#[macro_use]
extern crate quote;

mod error;
mod items;
mod json;
mod utils;

use error::{Error, Result};
use items::Item;
use proc_macro2::Span;

/// Arguments given to the `eth_abi` attribute macro.
struct Args {
    /// The required name of the endpoint.
    endpoint_name: String,
    /// The optional name of the client.
    client_name: Option<String>,
}

impl Args {
    /// Extracts `eth_abi` argument information from the given `syn::AttributeArgs`.
    pub fn from_attribute_args(attr_args: syn::AttributeArgs) -> Result<Args> {
        if attr_args.len() == 0 || attr_args.len() > 2 {
            return Err(Error::invalid_number_of_arguments(0));
        }
        let endpoint_name =
            if let syn::NestedMeta::Meta(syn::Meta::Word(ident)) = attr_args.get(0).unwrap() {
                Ok(ident.to_string())
            } else {
                Err(Error::malformatted_argument(0))
            }?;
        let client_name = attr_args
            .get(1)
            .map(|meta| {
                if let syn::NestedMeta::Meta(syn::Meta::Word(ident)) = meta {
                    Ok(ident.to_string())
                } else {
                    Err(Error::malformatted_argument(1))
                }
            })
            .map(|meta| meta.unwrap());
        Ok(Args {
            endpoint_name,
            client_name,
        })
    }

    /// Returns the given endpoint name.
    pub fn endpoint_name(&self) -> &str {
        &self.endpoint_name
    }

    /// Returns the optional client name.
    pub fn client_name(&self) -> Option<&str> {
        self.client_name.as_ref().map(|s| s.as_str())
    }
}

/// Derive of the Ethereum/Solidity ABI for the given trait interface.
///
/// The first parameter represents the identifier of the generated endpoint
/// implementation. The seconds parameter is optional and represents the
/// identifier of the generated client implementation.
///
/// # System Description
///
/// ## Endpoint
///
/// Converts ABI encoded payload into a called function with its parameters.
///
/// ## Client
///
/// Opposite of an endpoint that allows users (clients) to build up queries
/// in the form of a payload to functions of a contract by a generated interface.
///
/// # Example: Using just one argument
///
/// ```
/// #[eth_abi(Endpoint)]
/// trait Contract { }
/// ```
///
/// Creates an endpoint implementation named `Endpoint` for the
/// interface defined in the `Contract` trait.
///
/// # Example: Using two arguments
///
/// ```
/// #[eth_abi(Endpoint2, Client2)]
/// trait Contract2 { }
/// ```
///
/// Creates an endpoint implementation named `Endpoint2` and a
/// client implementation named `Client2` for the interface
/// defined in the `Contract2` trait.
#[proc_macro_attribute]
pub fn eth_abi(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let args_toks = parse_macro_input!(args as syn::AttributeArgs);
    let input_toks = parse_macro_input!(input as syn::Item);

    let output = match impl_eth_abi(args_toks, input_toks) {
        Ok(output) => output,
        Err(err) => panic!("[eth_abi] encountered error: {}", err),
    };

    output.into()
}

/// Implementation of `eth_abi`.
///
/// This convenience function is mainly used to better handle the results of token stream.
fn impl_eth_abi(args: syn::AttributeArgs, input: syn::Item) -> Result<proc_macro2::TokenStream> {
    let args = Args::from_attribute_args(args)?;
    let intf = items::Interface::from_item(input);

    crate::json::write_json_abi(&intf)?;

    match args.client_name() {
        None => generate_eth_endpoint_wrapper(&intf, args.endpoint_name()),
        Some(client_name) => {
            generate_eth_endpoint_and_client_wrapper(&intf, args.endpoint_name(), client_name)
        }
    }
}

/// Generates the eth abi code in case of a single provided endpoint.
fn generate_eth_endpoint_wrapper(
    intf: &items::Interface,
    endpoint_name: &str,
) -> Result<proc_macro2::TokenStream> {
    // FIXME: Code duplication with `generate_eth_endpoint_and_client_wrapper`
    //        We might want to fix this, however it is not critical.
    //        >>>
    let name_ident_use = syn::Ident::new(intf.name(), Span::call_site());
    let mod_name = format!("owasm_abi_impl_{}", &intf.name().clone());
    let mod_name_ident = syn::Ident::new(&mod_name, Span::call_site());
    // FIXME: <<<

    let endpoint_toks = generate_eth_endpoint(endpoint_name, intf);
    let endpoint_ident = syn::Ident::new(endpoint_name, Span::call_site());

    Ok(quote! {
        #intf
        #[allow(non_snake_case)]
        mod #mod_name_ident {
            use oasis_std::prelude::*;
            use super::#name_ident_use;
            #endpoint_toks
        }
        pub use self::#mod_name_ident::#endpoint_ident;
    })
}

/// Generates the eth abi code in case of a provided endpoint and client.
fn generate_eth_endpoint_and_client_wrapper(
    intf: &items::Interface,
    endpoint_name: &str,
    client_name: &str,
) -> Result<proc_macro2::TokenStream> {
    // FIXME: Code duplication with `generate_eth_endpoint_and_client_wrapper`
    //        We might want to fix this, however it is not critical.
    //        >>>
    let name_ident_use = syn::Ident::new(intf.name(), Span::call_site());
    let mod_name = format!("owasm_abi_impl_{}", &intf.name().clone());
    let mod_name_ident = syn::Ident::new(&mod_name, Span::call_site());
    // FIXME: <<<

    let endpoint_toks = generate_eth_endpoint(endpoint_name, &intf);
    let client_toks = generate_eth_client(client_name, &intf);
    let endpoint_name_ident = syn::Ident::new(endpoint_name, Span::call_site());
    let client_name_ident = syn::Ident::new(&client_name, Span::call_site());

    Ok(quote! {
        #intf
        #[allow(non_snake_case)]
        mod #mod_name_ident {
            use oasis_std::prelude::*;
            use super::#name_ident_use;
            #endpoint_toks
            #client_toks
        }
        pub use self::#mod_name_ident::#endpoint_name_ident;
        pub use self::#mod_name_ident::#client_name_ident;
    })
}

fn generate_eth_client(client_name: &str, intf: &items::Interface) -> proc_macro2::TokenStream {
    let client_ctor = intf.constructor().map(|signature| {
        utils::produce_signature(
            &signature.name,
            &signature.method_sig,
            quote! {
                #![allow(unused_mut)]
                #![allow(unused_variables)]
                unimplemented!()
            },
        )
    });

    let calls: Vec<proc_macro2::TokenStream> = intf.items().iter().filter_map(|item| {
		match *item {
			Item::Signature(ref signature)  => {
				let hash_literal = syn::Lit::Int(
					syn::LitInt::new(signature.hash as u64, syn::IntSuffix::U32, Span::call_site()));
				let argument_push: Vec<proc_macro2::TokenStream> = utils::iter_signature(&signature.method_sig)
					.map(|(pat, _)| quote! { sink.push(#pat); })
					.collect();
				let argument_count_literal = syn::Lit::Int(
					syn::LitInt::new(argument_push.len() as u64, syn::IntSuffix::Usize, Span::call_site()));

				let result_instance = match signature.method_sig.decl.output {
					syn::ReturnType::Default => quote!{
						let mut result = Vec::new();
					},
					syn::ReturnType::Type(_, _) => quote!{
						let mut result = [0u8; 32];
					},
				};

				let result_pop = match signature.method_sig.decl.output {
					syn::ReturnType::Default => None,
					syn::ReturnType::Type(_, _) => Some(
						quote!{
							let mut stream = oasis_std::abi::Stream::new(&result);
							stream.pop().expect("failed decode call output")
						}
					),
				};

				Some(utils::produce_signature(
					&signature.name,
					&signature.method_sig,
					quote!{
						#![allow(unused_mut)]
						#![allow(unused_variables)]
						let mut payload = Vec::with_capacity(4 + #argument_count_literal * 32);
						payload.push((#hash_literal >> 24) as u8);
						payload.push((#hash_literal >> 16) as u8);
						payload.push((#hash_literal >> 8) as u8);
						payload.push(#hash_literal as u8);

						let mut sink = oasis_std::abi::Sink::new(#argument_count_literal);
						#(#argument_push)*

						sink.drain_to(&mut payload);

						#result_instance

						oasis_std::ext::call(self.gas.unwrap_or(200000), &self.address, self.value.clone().unwrap_or(U256::zero()), &payload, &mut result[..])
							.expect("Call failed; todo: allow handling inside contracts");

						#result_pop
					}
				))
			},
			Item::Event(ref event)  => {
				Some(utils::produce_signature(
					&event.name,
					&event.method_sig,
					quote!{
						#![allow(unused_variables)]
						panic!("cannot use event in client interface");
					}
				))
			},
			_ => None,
		}
	}).collect();

    let client_ident = syn::Ident::new(client_name, Span::call_site());
    let name_ident = syn::Ident::new(intf.name(), Span::call_site());

    quote! {
        pub struct #client_ident {
            gas: Option<u64>,
            address: Address,
            value: Option<U256>,
        }

        impl #client_ident {
            pub fn new(address: Address) -> Self {
                #client_ident {
                    gas: None,
                    address: address,
                    value: None,
                }
            }

            pub fn gas(mut self, gas: u64) -> Self {
                self.gas = Some(gas);
                self
            }

            pub fn value(mut self, val: U256) -> Self {
                self.value = Some(val);
                self
            }
        }

        impl #name_ident for #client_ident {
            #client_ctor
            #(#calls)*
        }
    }
}

fn generate_eth_endpoint(endpoint_name: &str, intf: &items::Interface) -> proc_macro2::TokenStream {
    fn check_value_if_payable_toks(is_payable: bool) -> proc_macro2::TokenStream {
        if is_payable {
            return quote! {};
        }
        quote! {
            if oasis_std::ext::value() > 0.into() {
                panic!("Unable to accept value in non-payable constructor call");
            }
        }
    }

    let ctor_branch = intf.constructor().map(|signature| {
        let arg_types = signature
            .arguments
            .iter()
            .map(|&(_, ref ty)| quote! { #ty });
        let check_value_if_payable = check_value_if_payable_toks(signature.is_payable);
        quote! {
            #check_value_if_payable
            let mut stream = oasis_std::abi::Stream::new(payload);
            self.inner.constructor(
                #(stream.pop::<#arg_types>().expect("argument decoding failed")),*
            );
        }
    });

    let branches: Vec<proc_macro2::TokenStream> = intf
        .items()
        .iter()
        .filter_map(|item| match *item {
            Item::Signature(ref signature) => {
                let hash_literal = syn::Lit::Int(syn::LitInt::new(
                    signature.hash as u64,
                    syn::IntSuffix::U32,
                    Span::call_site(),
                ));
                let ident = &signature.name;
                let arg_types = signature
                    .arguments
                    .iter()
                    .map(|&(_, ref ty)| quote! { #ty });
                let check_value_if_payable = check_value_if_payable_toks(signature.is_payable);
                if !signature.return_types.is_empty() {
                    let return_count_literal = syn::Lit::Int(syn::LitInt::new(
                        signature.return_types.len() as u64,
                        syn::IntSuffix::Usize,
                        Span::call_site(),
                    ));
                    Some(quote! {
                        #hash_literal => {
                            #check_value_if_payable
                            let mut stream = oasis_std::abi::Stream::new(method_payload);
                            let result = inner.#ident(
                                #(stream.pop::<#arg_types>().expect("argument decoding failed")),*
                            );
                            let mut sink = oasis_std::abi::Sink::new(#return_count_literal);
                            sink.push(result);
                            sink.finalize_panicking()
                        }
                    })
                } else {
                    Some(quote! {
                        #hash_literal => {
                            #check_value_if_payable
                            let mut stream = oasis_std::abi::Stream::new(method_payload);
                            inner.#ident(
                                #(stream.pop::<#arg_types>().expect("argument decoding failed")),*
                            );
                            Vec::new()
                        }
                    })
                }
            }
            _ => None,
        })
        .collect();

    let endpoint_ident = syn::Ident::new(endpoint_name, Span::call_site());
    let name_ident = syn::Ident::new(&intf.name(), Span::call_site());

    quote! {
        pub struct #endpoint_ident<T: #name_ident> {
            pub inner: T,
        }

        impl<T: #name_ident> From<T> for #endpoint_ident<T> {
            fn from(inner: T) -> #endpoint_ident<T> {
                #endpoint_ident {
                    inner: inner,
                }
            }
        }

        impl<T: #name_ident> #endpoint_ident<T> {
            pub fn new(inner: T) -> Self {
                #endpoint_ident {
                    inner: inner,
                }
            }

            pub fn instance(&self) -> &T {
                &self.inner
            }
        }

        impl<T: #name_ident> oasis_std::abi::EndpointInterface for #endpoint_ident<T> {
            #[allow(unused_mut)]
            #[allow(unused_variables)]
            fn dispatch(&mut self, payload: &[u8]) -> Vec<u8> {
                let inner = &mut self.inner;
                if payload.len() < 4 {
                    panic!("Invalid abi invoke");
                }
                let method_id = ((payload[0] as u32) << 24)
                    + ((payload[1] as u32) << 16)
                    + ((payload[2] as u32) << 8)
                    + (payload[3] as u32);

                let method_payload = &payload[4..];

                match method_id {
                    #(#branches,)*
                    _ => panic!("Invalid method signature"),
                }
            }

            #[allow(unused_variables)]
            #[allow(unused_mut)]
            fn dispatch_ctor(&mut self, payload: &[u8]) {
                #ctor_branch
            }
        }
    }
}

include!("contract.rs");
