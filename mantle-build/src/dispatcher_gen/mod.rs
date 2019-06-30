use std::path::Path;

use syntax::{
    ast::{Arg, Block, Crate, Item, ItemKind, MethodSig, StmtKind},
    print::pprust,
    ptr::P,
};
use syntax_pos::symbol::Symbol;

use crate::parse;

pub fn generate_and_insert(
    krate: &mut Crate,
    out_dir: &Path,
    crate_name: &str,
    service_name: Symbol,
    ctor: &MethodSig,
    rpcs: Vec<(Symbol, MethodSig)>,
) {
    let (default_fn, rpcs): (Vec<_>, Vec<_>) = rpcs.into_iter().partition(is_default_fn);

    let default_fn_returns_result = default_fn
        .get(0)
        .map(|(_, sig)| crate::utils::unpack_syntax_ret(&sig.decl.output).is_result);

    if !rpcs.is_empty() {
        let rpcs_dispatcher =
            generate_rpc_dispatcher(service_name, &rpcs, default_fn_returns_result);
        let rpcs_include_file = out_dir.join(format!("{}_dispatcher.rs", crate_name));
        std::fs::write(
            &rpcs_include_file,
            pprust::block_to_string(&rpcs_dispatcher),
        )
        .unwrap();
        insert_rpc_dispatcher_stub(krate, &rpcs_include_file);
    }

    let ctor_fn = generate_ctor_fn(service_name, &ctor);
    let ctor_include_file = out_dir.join(format!("{}_ctor.rs", crate_name));
    std::fs::write(&ctor_include_file, pprust::item_to_string(&ctor_fn)).unwrap();
    krate.module.items.push(
        parse!(format!("include!(\"{}\");", ctor_include_file.display()) => parse_item).unwrap(),
    );
    krate.module.items.insert(0,
        parse!(r#"
            #[cfg(all(
                not(any(test, feature = "mantle-build-compiletest")),
                not(all(
                    target_arch = "wasm32",
                    not(target_env = "emscripten")
                ))
            ))]
            compile_error!("Compiling a Mantle service to a native target is unlikely to work as expected. Did you meant to use `cargo build --target wasm32-wasi`?");
        "# => parse_item).unwrap(),
    );
}

fn generate_rpc_dispatcher(
    service_name: Symbol,
    rpcs: &[(Symbol, MethodSig)],
    default_fn_returns_result: Option<bool>,
) -> P<Block> {
    let rpc_payload_variants = rpcs // e.g., `fn_name { input1: String, input2: Option<u64> }`
        .iter()
        .map(|(name, sig)| {
            format!(
                "{} {{ {} }}",
                name,
                structify_args(&sig.decl.inputs[2..]).join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    let rpc_match_arms = rpcs
        .iter()
        .map(|(name, sig)| {
            let arg_names = sig.decl.inputs[2..]
                .iter()
                .map(|arg| pprust::pat_to_string(&arg.pat))
                .collect::<Vec<_>>()
                .join(", ");
            if crate::utils::unpack_syntax_ret(&sig.decl.output).is_result {
                format!(
                    r#"RpcPayload::{name} {{ {arg_names} }} => {{
                        service.{name}(&ctx, {arg_names})
                            .map(|output| {{
                                mantle::reexports::serde_cbor::to_vec(&output).unwrap()
                            }})
                            .map_err(|err| format!("{{:?}}", err))
                    }}"#,
                    name = name,
                    arg_names = arg_names,
                )
            } else {
                format!(
                    r#"RpcPayload::{name} {{ {arg_names} }} => {{
                        let output = service.{name}(&ctx, {arg_names});
                        Ok(mantle::reexports::serde_cbor::to_vec(&output).unwrap())
                    }}"#,
                    name = name,
                    arg_names = arg_names,
                )
            }
        })
        .collect::<String>();

    let default_fn_arm = match default_fn_returns_result {
        Some(true) => {
            r#"_ => service.default(&ctx)
            .map(|output| Vec::new())
            .map_err(|err| format!("{:?}", err))"#
        }
        Some(false) => {
            r#"_ => {
                service.default(&ctx);
                Ok(Vec::new())
            }"#
        }
        None => "",
    };

    parse!(format!(r#"{{
        #[allow(warnings)]
        {{
            use mantle::reexports::serde::{{Serialize, Deserialize}};
            use mantle::Service as _;

            #[derive(Serialize, Deserialize)]
            #[serde(tag = "method", content = "payload")]
            enum RpcPayload {{
                {rpc_payload_variants}
            }}

            let ctx = mantle::Context::default(); // TODO(#33)
            let mut service = <{service_ident}>::coalesce();
            let payload: RpcPayload =
                mantle::reexports::serde_cbor::from_slice(&mantle::backend::input()).unwrap();
            let output = match payload {{
                {call_tree} // match arms return Result<Vec<u8>> (ABI-encoded bytes)
                {default_fn_arm}
            }};
            match output {{
                Ok(output) => mantle::backend::ret(&output),
                Err(err) => mantle::backend::err(&format!("{{:#?}}", err).into_bytes()),
            }}
        }}
        }}"#,
        rpc_payload_variants = rpc_payload_variants,
        service_ident = service_name.as_str().get(),
        call_tree = rpc_match_arms,
        default_fn_arm = default_fn_arm,
    ) => parse_block)
}

fn generate_ctor_fn(service_name: Symbol, ctor: &MethodSig) -> P<Item> {
    let ctor_arg_names = ctor.decl.inputs[1..]
        .iter()
        .map(|arg| pprust::pat_to_string(&arg.pat))
        .collect::<Vec<_>>()
        .join(", ");

    let ctor_payload_unpack = if ctor.decl.inputs.len() > 1 {
        format!(
            "let CtorPayload {{ {} }} =
                    mantle::reexports::serde_cbor::from_slice(&mantle::backend::input()).unwrap();",
            ctor_arg_names
        )
    } else {
        String::new()
    };

    let ctor_stmt = if crate::utils::unpack_syntax_ret(&ctor.decl.output).is_result {
        format!(
            r#"
            match <{service_ident}>::new(&ctx, {ctor_arg_names}) {{
                Ok(service) => service,
                Err(err) => {{
                    mantle::backend::err(&format!("{{:#?}}", err).into_bytes());
                    return 1;
                }}
            }}
            "#,
            service_ident = service_name.as_str().get(),
            ctor_arg_names = ctor_arg_names,
        )
    } else {
        format!(
            "<{service_ident}>::new(&ctx, {ctor_arg_names})",
            service_ident = service_name.as_str().get(),
            ctor_arg_names = ctor_arg_names
        )
    };

    parse!(format!(r#"
            #[allow(warnings)]
            #[no_mangle]
            extern "C" fn _mantle_deploy() -> u8 {{
                use mantle::Service as _;
                use mantle::reexports::serde::{{Serialize, Deserialize}};

                #[derive(Serialize, Deserialize)]
                #[allow(non_camel_case_types)]
                struct CtorPayload {{
                    {ctor_payload_types}
                }}
                let ctx = mantle::Context::default(); // TODO(#33)
                {ctor_payload_unpack}
                let mut service = {ctor_stmt};
                <{service_ident}>::sunder(service);
                return 0;
            }}
        "#,
        ctor_stmt = ctor_stmt,
        ctor_payload_unpack = ctor_payload_unpack,
        ctor_payload_types = structify_args(&ctor.decl.inputs[1..]).join(", "),
        service_ident = service_name.as_str().get(),
    ) => parse_item)
    .unwrap()
}

fn structify_args(args: &[Arg]) -> Vec<String> {
    args.iter()
        .map(|arg| {
            let pat_ident = pprust::ident_to_string(match arg.pat.node {
                syntax::ast::PatKind::Ident(_, ident, _) => ident,
                _ => unreachable!("Checked during visitation."),
            });
            format!("{}: {}", pat_ident, pprust::ty_to_string(&arg.ty))
        })
        .collect()
}

fn insert_rpc_dispatcher_stub(krate: &mut Crate, include_file: &Path) {
    for item in krate.module.items.iter_mut() {
        if item.ident.name != Symbol::intern("main") {
            continue;
        }
        let main_fn_block = match &mut item.node {
            ItemKind::Fn(_, _, _, ref mut block) => block,
            _ => continue,
        };
        let mantle_macro_idx = main_fn_block
            .stmts
            .iter()
            .position(|stmt| match &stmt.node {
                StmtKind::Mac(mac) => {
                    let mac_ = &mac.0.node;
                    crate::utils::path_ends_with(&mac_.path, &["mantle", "service"])
                }
                _ => false,
            })
            .unwrap();
        main_fn_block.stmts.splice(
            mantle_macro_idx..=mantle_macro_idx,
            parse!(format!("include!(\"{}\");", include_file.display()) => parse_stmt),
        );
        break;
    }
}

fn is_default_fn(rpc: &(Symbol, MethodSig)) -> bool {
    let (name, msig) = rpc;
    if name.as_str() != "default" {
        return false;
    }
    match msig.decl.inputs.as_slice() {
        [zelf, ctx] if zelf.is_self() && crate::utils::is_context_ref(&ctx.ty) => (),
        _ => return false,
    }
    match &crate::utils::unpack_syntax_ret(&msig.decl.output).ty {
        crate::utils::ReturnType::None => true,
        _ => false,
    }
}
