use std::path::Path;

use syntax::{
    ast::{Arg, Block, Crate, Item, ItemKind, MethodSig, StmtKind},
    print::pprust,
    ptr::P,
};
use syntax_pos::symbol::Symbol;

use crate::{
    parse,
    visitor::{ParsedRpc, ParsedRpcKind},
};

pub fn generate_and_insert(
    krate: &mut Crate,
    out_dir: &Path,
    crate_name: &str,
    service_name: Symbol,
    ctor: &MethodSig,
    rpcs: Vec<ParsedRpc>,
) {
    let default_fn = rpcs.iter().find(|rpc| match rpc.kind {
        ParsedRpcKind::Default(_) => true,
        _ => false,
    });

    if !rpcs.is_empty() {
        let rpcs_dispatcher = generate_rpc_dispatcher(service_name, &rpcs, default_fn);
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
    rpcs: &[ParsedRpc],
    default_fn: Option<&ParsedRpc>,
) -> P<Block> {
    let rpc_payload_variants = rpcs // e.g., `fn_name { input1: String, input2: Option<u64> }`
        .iter()
        .map(|rpc| {
            format!(
                "{} {{ {} }}",
                rpc.name,
                structify_args(&rpc.sig.decl.inputs[2..]).join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    let mut any_rpc_returns_result = false;
    let rpc_match_arms = rpcs
        .iter()
        .map(|rpc| {
            let arg_names = rpc.sig.decl.inputs[2..]
                .iter()
                .map(|arg| pprust::pat_to_string(&arg.pat))
                .collect::<Vec<_>>()
                .join(", ");
            if crate::utils::unpack_syntax_ret(&rpc.sig.decl.output).is_result {
                any_rpc_returns_result = true;
                format!(
                    r#"RpcPayload::{name} {{ {arg_names} }} => {{
                        service.{name}(&ctx, {arg_names})
                            .map(|output| {{
                                mantle::reexports::serde_cbor::to_vec(&output).unwrap()
                            }})
                            .map_err(|err| {{
                                mantle::reexports::serde_cbor::to_vec(&err).unwrap()
                            }})
                    }}"#,
                    name = rpc.name,
                    arg_names = arg_names,
                )
            } else {
                format!(
                    r#"RpcPayload::{name} {{ {arg_names} }} => {{
                        let output = service.{name}(&ctx, {arg_names});
                        Ok(mantle::reexports::serde_cbor::to_vec(&output).unwrap())
                    }}"#,
                    name = rpc.name,
                    arg_names = arg_names,
                )
            }
        })
        .collect::<String>();

    let default_fn_arm = if let Some(rpc) = default_fn {
        if crate::utils::unpack_syntax_ret(&rpc.sig.decl.output).is_result {
            any_rpc_returns_result = true;
            format!(
                r#"_ => service.{name}(&ctx)
                    .map(|output| {{
                        mantle::reexports::serde_cbor::to_vec(&output).unwrap()
                    }})
                    .map_err(|err| {{
                        mantle::reexports::serde_cbor::to_vec(&err).unwrap()
                    }})"#,
                name = rpc.name
            )
        } else {
            format!(
                r#"_ => {{
                    let output = service.{name}(&ctx);
                    Ok(mantle::reexports::serde_cbor::to_vec(&output).unwrap())
                }}"#,
                name = rpc.name
            )
        }
    } else {
        String::new()
    };

    let output_err_ty = if any_rpc_returns_result {
        "Vec<u8>"
    } else {
        "()"
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
            let output: Result<Vec<u8>, {output_err_ty}> = match payload {{
                {call_tree} // match arms return Result<Vec<u8>> (ABI-encoded bytes)
                {default_fn_arm}
            }};
            <{service_ident}>::sunder(service);
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
        output_err_ty = output_err_ty
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
