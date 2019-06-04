mod visitor;

use syntax::{
    ast::{Arg, Block, Item, MethodSig},
    parse::ParseSess,
    print::pprust,
    ptr::P,
};
use syntax_pos::symbol::Symbol;

// macro_rules! parse {
//     ($quotable:block using $parse_fn:ident) => {{
//         let src = format!("{}", quote!($quotable));
//         let sess = syntax::parse::ParseSess::new(syntax::source_map::FilePathMapping::empty());
//         let mut parser = syntax::parse::new_parser_from_source_str(
//             &sess,
//             syntax::source_map::FileName::Custom(String::new()),
//             src,
//         );
//         parser.$parse_fn().unwrap()
//     }};
// }

macro_rules! parse {
    ($src:expr => $parse_fn:ident) => {{
        let sess = syntax::parse::ParseSess::new(syntax::source_map::FilePathMapping::empty());
        let mut parser = syntax::parse::new_parser_from_source_str(
            &sess,
            syntax::source_map::FileName::Custom(String::new()),
            $src,
        );
        parser.$parse_fn().unwrap()
    }};
}

pub fn generate(
    sess: ParseSess,
    sigs: Vec<(Symbol, &MethodSig)>,
) -> (
    Option<P<Item>>, /* ctor fn */
    P<Block>,        /* dispatch tree */
) {
    let ctor = sigs.iter().find(|(name, _)| *name == Symbol::intern("new"));

    let dispatch_tree = parse!(r#"{
        let a = "hello, world!";
    }"#.to_string() => parse_block);

    let dispatch_export = ctor.map(|(_, sig)| {
        let ctor_body = "";

        parse!(format!(r#"
            #[no_mangle]
            extern "C" fn _mantle_ctor() {{
                {}
            }}
        "#, ctor_body) => parse_item)
        .unwrap()
    });

    (dispatch_export, dispatch_tree)
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
