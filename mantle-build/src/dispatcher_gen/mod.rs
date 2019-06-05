use syntax::{
    ast::{Arg, Block, Crate, Item, ItemKind, MethodSig, StmtKind},
    print::pprust,
    ptr::P,
};
use syntax_pos::symbol::Symbol;

use crate::parse;

pub struct Dispatchers {
    pub ctor_fn: P<Item>,
    pub rpc_dispatcher: P<Block>,
}

pub fn generate(
    service_name: Symbol,
    ctor: &MethodSig,
    rpcs: Vec<(Symbol, &MethodSig)>,
) -> Dispatchers {
    let rpc_payload_types = rpcs // e.g., `fn_name { input1: String, input2: Option<u64> }`
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
            format!(
                r#"RpcPayload::{name} {{ {arg_names} }} => {{
                    let result = service.{name}(&ctx, {arg_names});
                    mantle::reexports::serde_cbor::to_vec(&result.map_err(|err| err.to_string())) // TODO(#15)
                }}"#,
                name = name,
                arg_names = arg_names,
            )
        })
        .collect::<String>();

    let rpc_dispatcher = parse!(format!(r#"{{
            #[derive(serde::Serialize, serde::Deserialize)]
            #[serde(tag = "method", content = "payload")]
            #[allow(non_camel_case_types)]
            pub enum RpcPayload<'a> {{
                {rpc_payload_types}
            }}

            let ctx = mantle::Context::default(); // TODO(#33)
            let mut service = <{service_ident}>::coalesce();
            use std::io::{{Read as _, Write as _}};
            let payload: RpcPayload =
                mantle::reexports::serde_cbor::from_reader(std::io::stdin()).unwrap();
            let serialized_result = match payload {{
                {call_tree}
            }}.unwrap();
            <{service_ident}>::sunder(service);
            std::io::stdout().write_all(&serialized_result);
        }}"#,
        rpc_payload_types = rpc_payload_types,
        service_ident = service_name.as_str().get(),
        call_tree = rpc_match_arms,
    ) => parse_block);

    let ctor_body = "";
    let ctor_fn = parse!(format!(r#"
            #[no_mangle]
            extern "C" fn _mantle_ctor() {{
                {}
            }}
        "#, ctor_body) => parse_item)
    .unwrap();

    Dispatchers {
        ctor_fn,
        rpc_dispatcher,
    }
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

pub fn insert_rpc_dispatcher(krate: &mut Crate, rpc_dispatcher: P<Block>) {
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
            rpc_dispatcher.into_inner().stmts,
        );
        break;
    }
}
