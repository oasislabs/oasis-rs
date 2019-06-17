cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "wasm32", target_os = "wasi"))] {
        mod wasi;
        use wasi as imp;
    } else {
        mod ext;
        use ext as imp;
    }
}

pub use imp::{
    address, balance, code, emit, err, input, payer, read, ret, sender, transact, value, write,
};
