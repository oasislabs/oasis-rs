thread_local! {
    static TEST_EXT_DEFS: Vec<TestExtDef> = {
        let fields: syn::FieldsNamed = parse_quote!({
            address: &Address,
            input: &[u8],
            r#return: &[u8],
            sender: &Address,
            value: &U256,
            gas: &U256,
        });
        fields.named
            .into_iter()
            .map(|field| {
                let ident = unraw(field.ident.as_ref().unwrap());

                let mut owned_ty = field.ty.clone();
                Deborrower {}.visit_type_mut(&mut owned_ty);

                TestExtDef {
                    bytes_arg: format_ident!("{}_bytes", ident),
                    bytes_len_arg: format_ident!("{}_len", ident),
                    push_fn_ident: format_ident!("push_{}", ident),
                    pop_fn_ident: format_ident!("pop_{}", ident),
                    upper_field: format_ident!("{}", ident.to_string().to_uppercase()),
                    field: field.ident.unwrap(),
                    owned_ty,
                    ty: field.ty,
                }
            })
        .collect()
    };
}

struct TestExtDef {
    field: syn::Ident,
    upper_field: syn::Ident,
    bytes_arg: syn::Ident,
    bytes_len_arg: syn::Ident,
    push_fn_ident: syn::Ident,
    pop_fn_ident: syn::Ident,
    ty: syn::Type,
    owned_ty: syn::Type,
}

impl TestExtDef {
    // For some reason, TokenStream can't go into thread_local.
    pub fn ext_push_fn_sig(&self) -> proc_macro2::TokenStream {
        let Self {
            bytes_arg,
            bytes_len_arg,
            push_fn_ident,
            ..
        } = self;
        let mut args = vec![quote! { #bytes_arg: *const u8 }];
        if self.ty_is_slice() {
            args.push(quote! { #bytes_len_arg: usize })
        }
        quote! { fn #push_fn_ident(#(#args),*) }
    }

    pub fn ext_push_fn_call(&self) -> proc_macro2::TokenStream {
        let Self {
            field,
            push_fn_ident,
            ..
        } = self;
        let mut call_args = vec![quote! { #field.as_ptr() }];
        if self.ty_is_slice() {
            call_args.push(quote! { #field.len() })
        }
        quote! { #push_fn_ident(#(#call_args),*) }
    }

    pub fn ty_is_slice(&self) -> bool {
        match self.ty {
            syn::Type::Reference(syn::TypeReference {
                elem: box syn::Type::Slice(_),
                ..
            }) => true,
            _ => false,
        }
    }
}

#[proc_macro]
/// Generates push/pop externs, mock externs, and bindings for a testing client.
/// @see `oasis_std::testing` for an example of use.
pub fn test_client(_input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    TEST_EXT_DEFS.with(|defs| {
        let ext_mod_ident: syn::Ident = parse_quote!(call_ctx_ext);
        let mock_ext_mod_ident = format_ident!("mock_{}", ext_mod_ident);

        let extern_fns = defs.iter().map(|def| {
            let TestExtDef { pop_fn_ident, .. } = def;
            let ext_push_fn_sig = def.ext_push_fn_sig();
            quote! {
                pub #ext_push_fn_sig;
                pub fn #pop_fn_ident();
            }
        });

        let mock_extern_fns = defs.iter().map(|def| {
            let TestExtDef { pop_fn_ident, .. } = def;
            let ext_push_fn_sig = def.ext_push_fn_sig();
            quote! {
                #[no_mangle]
                #[linkage = "weak"]
                extern "C" #ext_push_fn_sig {} // nop

                #[no_mangle]
                #[linkage = "weak"]
                extern "C" fn #pop_fn_ident() {} // nop
            }
        });

        let bindings = defs.iter().map(|def| {
            let TestExtDef {
                push_fn_ident,
                pop_fn_ident,
                field,
                ty,
                ..
            } = def;
            let ext_push_fn_call = def.ext_push_fn_call();
            quote! {
                pub fn #push_fn_ident(#field: #ty) {
                    unsafe { #ext_mod_ident::#ext_push_fn_call; }
                }

                pub fn #pop_fn_ident() {
                    unsafe { #ext_mod_ident::#pop_fn_ident(); }
                }
            }
        });

        proc_macro::TokenStream::from(quote! {
            mod #ext_mod_ident {
                extern "C" {
                    #(#extern_fns)*

                    pub fn push_current_address_as_sender();
                }
            }

            mod #mock_ext_mod_ident {
                #(#mock_extern_fns)*

                #[no_mangle]
                #[linkage = "weak"]
                extern "C" fn push_current_address_as_sender() {}
            }

            #(#bindings)*

            pub fn push_current_address_as_sender() {
                unsafe { #ext_mod_ident::push_current_address_as_sender(); }
            }
        })
    })
}

#[proc_macro]
/// Generates push/pop externs and impls. Used in `oasis_test::ext`.
pub fn test_host(_input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    TEST_EXT_DEFS.with(|defs| {
        let call_envs = defs.iter().map(|def| {
            let TestExtDef {
                upper_field,
                owned_ty,
                ..
            } = def;
            quote! {
                static #upper_field: RefCell<Vec<#owned_ty>> = RefCell::new(Vec::new());
            }
        });

        let pp_receivers = defs.iter().map(|def| {
            let TestExtDef {
                upper_field,
                bytes_arg,
                bytes_len_arg,
                pop_fn_ident,
                owned_ty,
                ..
            } = def;
            let rety = if def.ty_is_slice() {
                quote! {
                    unsafe { std::slice::from_raw_parts(#bytes_arg, #bytes_len_arg) }.to_vec()
                }
            } else {
                quote! { unsafe { #owned_ty::from_raw(#bytes_arg) } }
            };
            let ext_push_fn_sig = def.ext_push_fn_sig();
            quote! {
                #[no_mangle]
                pub(super) extern "C" #ext_push_fn_sig {
                    #upper_field.with(|field| field.borrow_mut().push(#rety));
                }

                #[no_mangle]
                pub(super) extern "C" fn #pop_fn_ident() {
                    #upper_field.with(|field| field.borrow_mut().pop());
                }
            }
        });

        let eth_accessors = defs.iter().map(|def| {
            let TestExtDef {
                field,
                upper_field,
                owned_ty,
                ..
            } = def;

            let (accessor_ident, len, len_accessor) = if def.ty_is_slice() {
                let field = unraw(field);
                let len_accessor_fn_ident = format_ident!("{}_length", field);
                (
                    format_ident!("fetch_{}", field),
                    quote! { #len_accessor_fn_ident() as usize },
                    Some(quote! {
                        #[no_mangle]
                        pub fn #len_accessor_fn_ident() -> u32 {
                            #upper_field.with(|field| field.borrow().last().unwrap().len() as u32)
                        }
                    }),
                )
            } else {
                (
                    field.clone(),
                    quote! { std::mem::size_of::<#owned_ty>() },
                    None,
                )
            };

            quote! {
                #[no_mangle]
                pub fn #accessor_ident(dest: *mut  u8) {
                    #upper_field.with(|field| unsafe {
                        dest.copy_from_nonoverlapping(
                            field.borrow().last().unwrap().as_ptr(),
                            #len
                        );
                    });
                }

                #len_accessor
            }
        });

        proc_macro::TokenStream::from(quote! {
            thread_local! {
                #(#call_envs)*
            }

            mod pp_receivers {
                use super::*;
                #(#pp_receivers)*

                #[no_mangle]
                extern "C" fn push_current_address_as_sender() {
                    SENDER.with(|sender| {
                        let addr = ADDRESS.with(|addr| *addr.borrow().last().unwrap());
                        sender.borrow_mut().push(addr);
                    });
                }
            }

            #(#eth_accessors)*
        })
    })
}
