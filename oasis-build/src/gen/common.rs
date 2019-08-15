use proc_macro2::{Literal, TokenStream};
use quote::quote;
use syntax::{ast, ptr::P, source_map};

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
        Type::String => quote!(&str),
        Type::Address => quote!(oasis_std::Address),
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
    let tyq = match ty {
        Type::Bool
        | Type::U8
        | Type::I8
        | Type::U16
        | Type::I16
        | Type::U32
        | Type::I32
        | Type::U64
        | Type::I64
        | Type::F32
        | Type::F64 => {
            return quote_ty(ty);
        }
        Type::Bytes => quote!([u8]),
        Type::String => quote!(str),
        Type::List(ty) => {
            let tyq = quote_ty(ty);
            quote!([#tyq])
        }
        _ => quote_ty(ty),
    };
    quote!(impl std::borrow::Borrow<#tyq>)
}

pub fn gen_include_item(include_path: impl AsRef<std::path::Path>) -> P<ast::Item> {
    P(ast::Item {
        ident: ast::Ident::from_str(""),
        attrs: Vec::new(),
        id: ast::DUMMY_NODE_ID,
        node: ast::ItemKind::Mac(source_map::dummy_spanned(gen_include_mac(include_path))),
        vis: source_map::dummy_spanned(ast::VisibilityKind::Public),
        span: syntax_pos::DUMMY_SP,
        tokens: None,
    })
}

pub fn gen_include_mac(include_path: impl AsRef<std::path::Path>) -> ast::Mac_ {
    use syntax::parse::token::{LitKind, Token, TokenKind};
    ast::Mac_ {
        path: ast::Path::from_ident(ast::Ident::from_str("include")),
        delim: ast::MacDelimiter::Parenthesis,
        tts: syntax::tokenstream::TokenTree::Token(Token {
            kind: TokenKind::lit(
                LitKind::Str,
                syntax_pos::Symbol::intern(&format!("{}", include_path.as_ref().display())),
                None,
            ),
            span: syntax_pos::DUMMY_SP,
        })
        .into(),
    }
}

pub fn gen_call_stmt(fn_ident: source_map::symbol::Ident) -> ast::Stmt {
    let call_ident = ast::Expr {
        node: ast::ExprKind::Path(None /* qself */, ast::Path::from_ident(fn_ident)),
        id: ast::DUMMY_NODE_ID,
        span: syntax_pos::DUMMY_SP,
        attrs: Default::default(),
    };
    let call_expr = ast::Expr {
        node: ast::ExprKind::Call(P(call_ident), Vec::new() /* args */),
        id: ast::DUMMY_NODE_ID,
        span: syntax_pos::DUMMY_SP,
        attrs: Default::default(),
    };
    ast::Stmt {
        node: ast::StmtKind::Semi(P(call_expr)),
        id: ast::DUMMY_NODE_ID,
        span: syntax_pos::DUMMY_SP,
    }
}

pub fn write_include(path: &std::path::Path, contents: &str) {
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
