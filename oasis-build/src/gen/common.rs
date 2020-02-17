use proc_macro2::{Literal, TokenStream};
use quote::quote;
use rustc_span::{source_map::dummy_spanned, DUMMY_SP};
use syntax::{ast, node_id::DUMMY_NODE_ID, ptr::P};

#[macro_export]
macro_rules! format_ident {
    ($fmt_str:literal, $($fmt_arg:expr),+) => {
        proc_macro2::Ident::new(&format!($fmt_str, $($fmt_arg),+), proc_macro2::Span::call_site())
    }
}

pub fn sanitize_ident(ident: &str) -> String {
    ident
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect()
}

pub fn quote_ty(ty: &oasis_rpc::Type) -> TokenStream {
    use oasis_rpc::Type;
    match ty {
        Type::Bool => quote!(bool),
        Type::U8 => quote!(u8),
        Type::I8 => quote!(i8),
        Type::U16 => quote!(u16),
        Type::I16 => quote!(i16),
        Type::U32 => quote!(u32),
        Type::I32 => quote!(i32),
        Type::U64 => quote!(u64),
        Type::I64 => quote!(i64),
        Type::F32 => quote!(f32),
        Type::F64 => quote!(f64),
        Type::Bytes => quote!(Vec<u8>),
        Type::String => quote!(String),
        Type::Address => quote!(oasis_std::Address),
        Type::Balance => quote!(oasis_std::Balance),
        Type::Defined { namespace, ty } => {
            let tyq = format_ident!("{}", ty);
            match namespace {
                Some(namespace) => {
                    let ns = format_ident!("{}", namespace);
                    quote!(#ns::#tyq)
                }
                None => quote!(#tyq),
            }
        }
        Type::Tuple(tys) => {
            let tyqs = tys.iter().map(quote_ty);
            quote!(( #(#tyqs),*) )
        }
        Type::Array(ty, count) => {
            let tyq = quote_ty(ty);
            let count = Literal::usize_suffixed(*count as usize);
            quote!([#tyq; #count])
        }
        Type::List(ty) => {
            let tyq = quote_ty(ty);
            quote!(Vec<#tyq>)
        }
        Type::Set(ty) => {
            let tyq = quote_ty(ty);
            quote!(std::collections::HashSet<#tyq>)
        }
        Type::Map(kty, vty) => {
            let ktyq = quote_ty(kty);
            let vtyq = quote_ty(vty);
            quote!(std::collections::HashMap<#ktyq, #vtyq>)
        }
        Type::Optional(ty) => {
            let tyq = quote_ty(ty);
            quote!(Option<#tyq>)
        }
        Type::Result(ok_ty, err_ty) => {
            let ok_tyq = quote_ty(ok_ty);
            let err_tyq = quote_ty(err_ty);
            quote!(Result<#ok_tyq, #err_tyq>)
        }
    }
}

pub fn quote_borrow(ty: &oasis_rpc::Type) -> TokenStream {
    use oasis_rpc::Type;
    match ty {
        Type::Bytes => quote!(&[u8]),
        Type::String => quote!(&str),
        Type::List(ty) => {
            let tyq = quote_ty(ty);
            quote!(&[#tyq])
        }
        _ => quote_ty(ty),
    }
}

pub fn gen_include_item(include_path: impl AsRef<std::path::Path>) -> P<ast::Item> {
    P(ast::Item {
        ident: ast::Ident::from_str(""),
        attrs: Vec::new(),
        id: DUMMY_NODE_ID,
        kind: ast::ItemKind::Mac(gen_include_mac(include_path)),
        vis: dummy_spanned(ast::VisibilityKind::Public),
        span: DUMMY_SP,
        tokens: None,
    })
}

pub fn gen_include_mac(include_path: impl AsRef<std::path::Path>) -> ast::Mac {
    use syntax::token::{LitKind, Token, TokenKind};
    ast::Mac {
        path: ast::Path::from_ident(ast::Ident::from_str("include")),
        args: P(ast::MacArgs::Delimited(
            syntax::tokenstream::DelimSpan::dummy(),
            ast::MacDelimiter::Parenthesis,
            syntax::tokenstream::TokenTree::Token(Token {
                kind: TokenKind::lit(
                    LitKind::Str,
                    rustc_span::Symbol::intern(&format!("{}", include_path.as_ref().display())),
                    None,
                ),
                span: rustc_span::DUMMY_SP,
            })
            .into(),
        )),
        prior_type_ascription: None,
    }
}

pub fn gen_call_stmt(fn_ident: rustc_span::source_map::symbol::Ident) -> ast::Stmt {
    let call_ident = ast::Expr {
        kind: ast::ExprKind::Path(None /* qself */, ast::Path::from_ident(fn_ident)),
        id: DUMMY_NODE_ID,
        span: DUMMY_SP,
        attrs: Default::default(),
    };
    let call_expr = ast::Expr {
        kind: ast::ExprKind::Call(P(call_ident), Vec::new() /* args */),
        id: DUMMY_NODE_ID,
        span: DUMMY_SP,
        attrs: Default::default(),
    };
    ast::Stmt {
        kind: ast::StmtKind::Semi(P(call_expr)),
        id: DUMMY_NODE_ID,
        span: DUMMY_SP,
    }
}

#[macro_export]
macro_rules! hash {
    ($( $arg:expr ),+) => {{
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        $( std::hash::Hash::hash(&$arg, &mut hasher); )+
        std::hash::Hasher::finish(&hasher)
    }}
}

pub fn write_generated(path: &std::path::Path, contents: &str) {
    if path.exists() {
        return;
    }
    std::fs::write(path, contents).unwrap();
    std::process::Command::new("rustfmt")
        .args(&[
            path.to_str().unwrap(),
            "--edition",
            "2018",
            "--emit",
            "files",
        ])
        .output()
        .ok();
}
