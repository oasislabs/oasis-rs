use std::{io::Write, path::Path};

use syntax::{
    ast::{Arg, Block, Crate, Item, ItemKind, MethodSig, StmtKind},
    print::pprust,
    ptr::P,
};
use syntax_pos::symbol::Symbol;

use crate::{
    parse,
    visitor::syntax::{ParsedRpc, ParsedRpcKind},
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
        let mut buf = Vec::new();
        writeln!(&mut buf, "{{").unwrap();
        for stmt in rpcs_dispatcher.stmts.iter() {
            writeln!(&mut buf, "{}", pprust::stmt_to_string(&stmt)).unwrap();
        }
        writeln!(&mut buf, "}}").unwrap();
        std::fs::write(&rpcs_include_file, &buf).unwrap();
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
                not(any(test, feature = "oasis-build-compiletest")),
                not(all(
                    target_arch = "wasm32",
                    not(target_env = "emscripten")
                ))
            ))]
            compile_error!("Compiling a Oasis service to a native target is unlikely to work as expected. Did you mean to use `cargo build --target wasm32-wasi`?");
        "# => parse_item).unwrap(),
    );
}

fn generate_rpc_dispatcher(
    service_name: Symbol,
    rpcs: &[ParsedRpc],
    default_fn: Option<&ParsedRpc>,
) -> P<Block> {
    let mut any_rpc_returns_result = false;
    let mut rpc_payload_variants = Vec::with_capacity(rpcs.len());
    let rpc_match_arms = rpcs
        .iter()
        .map(|rpc| {
            let (arg_names, arg_tys) = split_args(&rpc.sig.decl.inputs[2..]);

            rpc_payload_variants.push(format!("{}({})", rpc.name, tuplize(&arg_tys)));

            if crate::utils::unpack_syntax_ret(&rpc.sig.decl.output).is_result {
                any_rpc_returns_result = true;
                gen_result_dispatch(rpc.name, arg_names)
            } else {
                gen_dispatch(rpc.name, arg_names)
            }
        })
        .collect::<String>();

    let default_fn_arm = if let Some(rpc) = default_fn {
        if crate::utils::unpack_syntax_ret(&rpc.sig.decl.output).is_result {
            any_rpc_returns_result = true;
            gen_result_dispatch(rpc.name, Vec::new())
        } else {
            gen_dispatch(rpc.name, Vec::new())
        }
    } else {
        String::new()
    };

    let output_err_ty = if any_rpc_returns_result {
        "Vec<u8>"
    } else {
        "()"
    };

    let err_returner = if any_rpc_returns_result {
        "oasis_std::backend::err(&err_output)"
    } else {
        r#"unreachable!("No RPC function returns Err")"#
    };

    parse!(format!(r#"{{
        #[allow(warnings)]
        {{
            use oasis_std::reexports::serde::{{Serialize, Deserialize}};
            use oasis_std::Service as _;

            #[derive(Serialize, Deserialize)]
            #[serde(tag = "method", content = "payload")]
            enum RpcPayload {{
                {rpc_payload_variants}
            }}

            let ctx = oasis_std::Context::default(); // TODO(#33)
            let mut service = <{service_ident}>::coalesce();
            let payload: RpcPayload =
                oasis_std::reexports::serde_cbor::from_slice(&oasis_std::backend::input()).unwrap();
            let output: std::result::Result<Vec<u8>, {output_err_ty}> = match payload {{
                {call_tree}
                {default_fn_arm}
            }};
            <{service_ident}>::sunder(service);
            match output {{
                Ok(output) => oasis_std::backend::ret(&output),
                Err(err_output) => {err_returner},
            }}
        }}
        }}"#,
        rpc_payload_variants = rpc_payload_variants.join(", "),
        service_ident = service_name.as_str().get(),
        call_tree = rpc_match_arms,
        default_fn_arm = default_fn_arm,
        output_err_ty = output_err_ty,
        err_returner = err_returner
    ) => parse_block)
}

fn gen_result_dispatch(name: Symbol, arg_names: Vec<String>) -> String {
    format!(
        r#"RpcPayload::{name}({tup_arg_names}) => match service.{name}(&ctx, {arg_names}) {{
            Ok(output) => Ok(oasis_std::reexports::serde_cbor::to_vec(&output).unwrap()),
            Err(err) => Err(oasis_std::reexports::serde_cbor::to_vec(&err).unwrap()),
        }}"#,
        name = name,
        tup_arg_names = tuplize(&arg_names),
        arg_names = arg_names.join(","),
    )
}

fn gen_dispatch(name: Symbol, arg_names: Vec<String>) -> String {
    format!(
        r#"RpcPayload::{name}({tup_arg_names}) => {{
            Ok(oasis_std::reexports::serde_cbor::to_vec(&service.{name}(&ctx, {arg_names})).unwrap())
        }}"#,
        name = name,
        tup_arg_names = tuplize(&arg_names),
        arg_names = arg_names.join(","),
    )
}

fn generate_ctor_fn(service_name: Symbol, ctor: &MethodSig) -> P<Item> {
    let (arg_names, arg_tys) = split_args(&ctor.decl.inputs[1..]);

    let ctor_payload_unpack = if ctor.decl.inputs.len() > 1 {
        format!(
            "let CtorPayload({}) =
                    oasis_std::reexports::serde_cbor::from_slice(&oasis_std::backend::input()).unwrap();",
            tuplize(&arg_names)
        )
    } else {
        String::new()
    };

    let ctor_stmt = if crate::utils::unpack_syntax_ret(&ctor.decl.output).is_result {
        format!(
            r#"
            match <{service_ident}>::new(&ctx, {arg_names}) {{
                Ok(service) => service,
                Err(err) => {{
                    oasis_std::backend::err(&format!("{{:#?}}", err).into_bytes());
                    return 1;
                }}
            }}
            "#,
            service_ident = service_name.as_str().get(),
            arg_names = arg_names.join(","),
        )
    } else {
        format!(
            "<{service_ident}>::new(&ctx, {arg_names})",
            service_ident = service_name.as_str().get(),
            arg_names = arg_names.join(",")
        )
    };

    parse!(format!(r#"
            #[allow(warnings)]
            #[no_mangle]
            extern "C" fn _oasis_deploy() -> u8 {{
                use oasis_std::Service as _;
                use oasis_std::reexports::serde::{{Serialize, Deserialize}};

                #[derive(Serialize, Deserialize)]
                #[allow(non_camel_case_types)]
                struct CtorPayload({ctor_payload_types});

                let ctx = oasis_std::Context::default(); // TODO(#33)
                {ctor_payload_unpack}
                let mut service = {ctor_stmt};
                <{service_ident}>::sunder(service);
                return 0;
            }}
        "#,
        ctor_stmt = ctor_stmt,
        ctor_payload_unpack = ctor_payload_unpack,
        ctor_payload_types = tuplize(&arg_tys),
        service_ident = service_name.as_str().get(),
    ) => parse_item)
    .unwrap()
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
        let oasis_macro_idx = main_fn_block
            .stmts
            .iter()
            .position(|stmt| match &stmt.node {
                StmtKind::Mac(mac) => {
                    let mac_ = &mac.0.node;
                    crate::utils::path_ends_with(&mac_.path, &["oasis_std", "service"])
                }
                _ => false,
            })
            .unwrap();
        main_fn_block.stmts.splice(
            oasis_macro_idx..=oasis_macro_idx,
            parse!(format!("include!(\"{}\");", include_file.display()) => parse_stmt),
        );
        break;
    }
}

fn split_args(args: &[Arg]) -> (Vec<String>, Vec<String>) {
    args.iter()
        .map(|arg| {
            (
                match arg.pat.node {
                    syntax::ast::PatKind::Ident(_, ident, _) => ident,
                    _ => unreachable!("Checked during visitation."),
                }
                .to_string(),
                pprust::ty_to_string(&arg.ty),
            )
        })
        .unzip()
}

/// Turns a non-empty sequence of items into a stringified tuple, else returns an empty string.
/// Tuplizing is necessary so that serde deserializes length-1 sequences into a newtype variant.
fn tuplize(items: &[String]) -> String {
    if items.is_empty() {
        String::new()
    } else {
        format!("({},)", items.join(","))
    }
}
