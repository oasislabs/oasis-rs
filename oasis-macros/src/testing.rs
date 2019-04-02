struct TestExtDef {
    push_fn: syn::Ident,
    pop_fn: syn::Ident,
    bytes_arg: syn::Ident,
    binding_arg: syn::Ident,
    ty: syn::Type,
}

#[proc_macro]
/// Generates push/pop externs, mock externs, and impls. Used in `oasis_std::testing`.
pub fn define_test_pp(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ext_mod_ident: syn::Ident = parse_quote!(call_ctx_ext);
    let mock_ext_mod_ident = format_ident!("mock_{}", ext_mod_ident);

    let defs: Vec<TestExtDef> = parse_macro_input!(input as syn::FieldsNamed)
        .named
        .into_iter()
        .map(|field| {
            let ident = field.ident.unwrap();
            TestExtDef {
                push_fn: format_ident!("push_{}", ident),
                pop_fn: format_ident!("pop_{}", ident),
                bytes_arg: format_ident!("{}_bytes", ident),
                binding_arg: ident,
                ty: field.ty,
            }
        })
        .collect();

    let extern_fns = defs.iter().map(|def| {
        let TestExtDef {
            push_fn,
            pop_fn,
            bytes_arg,
            ..
        } = def;
        quote! {
            pub fn #push_fn(#bytes_arg: *const u8);
            pub fn #pop_fn();
        }
    });

    let mock_extern_fns = defs.iter().map(|def| {
        let TestExtDef {
            push_fn,
            pop_fn,
            bytes_arg,
            ..
        } = def;
        quote! {
            #[no_mangle]
            #[linkage = "extern_weak"]
            extern "C" fn #push_fn(#bytes_arg: *const u8) {} // nop

            #[no_mangle]
            #[linkage = "extern_weak"]
            extern "C" fn #pop_fn() {} // nop
        }
    });

    let bindings = defs.iter().map(|def| {
        let TestExtDef {
            push_fn,
            pop_fn,
            binding_arg,
            ty,
            ..
        } = def;
        quote! {
            pub fn #push_fn(#binding_arg: #ty) {
                unsafe { #ext_mod_ident::#push_fn(#binding_arg.as_ptr()); }
            }

            pub fn #pop_fn() {
                unsafe { #ext_mod_ident::#pop_fn(); }
            }
        }
    });

    proc_macro::TokenStream::from(quote! {
        mod #ext_mod_ident {
            extern "C" {
                #(#extern_fns)*
            }
        }

        mod #mock_ext_mod_ident {
            #(#mock_extern_fns)*
        }

        #(#bindings)*
    })
}
