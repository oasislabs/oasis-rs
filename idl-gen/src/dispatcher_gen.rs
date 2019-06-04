use syntax::{
    ast::{Block, Item, MethodSig},
    ptr::P,
};
use syntax_pos::symbol::Symbol;

macro_rules! parse {
    ($quotable:block using $parse_fn:ident) => {{
        let src = format!("{}", quote!($quotable));
        let sess = syntax::parse::ParseSess::new(syntax::source_map::FilePathMapping::empty());
        let mut parser = syntax::parse::new_parser_from_source_str(
            &sess,
            syntax::source_map::FileName::Custom(String::new()),
            src,
        );
        parser.$parse_fn().unwrap()
    }};
}

pub fn generate(
    sigs: Vec<(Symbol, &MethodSig)>,
) -> (
    Option<P<Item>>, /* ctor fn */
    P<Block>,        /* dispatch tree */
) {
    let ctor = sigs.iter().find(|(name, _)| *name == Symbol::intern("new"));

    let dispatch_tree = parse!({{
        let a = "hello, world!";
    }} using parse_block);

    let dispatch_export = ctor.map(|(_, sig)| {
        parse!({
            #[no_mangle]
            extern "C" fn _mantle_ctor() {
            }
        } using parse_item)
        .unwrap()
    });

    (dispatch_export, dispatch_tree)
}
