#[proc_macro_attribute]
pub fn contract(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let contract = crate::utils::Contract::new(&parse_macro_input!(input as syn::ItemTrait));

    let contract_struct = contract.struct_name;
    let trait_name = contract.trait_name;
    let contract_ep = contract.endpoint_name;
    let contract_client = contract.client_name;
    let method_sigs = contract.method_sigs;
    let method_impls = contract.method_impls;

    proc_macro::TokenStream::from(quote! {
      use oasis_std::{
          abi::EndpointInterface,
          prelude::*,
      };

      #[oasis_std::derive::eth_abi(#contract_ep, #contract_client)]
      pub trait #trait_name {
        #(#method_sigs)*
      }

      pub struct #contract_struct;

      impl #trait_name for #contract_struct {
        #(#method_impls)*
      }

      #[no_mangle]
      pub fn deploy() {
        let mut endpoint = #contract_ep::new(#contract_struct {});
        endpoint.dispatch_ctor(&oasis_std::ext::input());
      }

      #[no_mangle]
      pub fn call() {
        let mut endpoint = #contract_ep::new(#contract_struct {});
        oasis_std::ext::ret(&endpoint.dispatch(&oasis_std::ext::input()));
      }
    })
}
