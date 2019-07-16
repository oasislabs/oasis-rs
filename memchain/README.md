# memchain

[![Latest Version](https://img.shields.io/crates/v/memchain.svg)](https://crates.io/crates/memchain)
[![docs](https://docs.rs/memchain/badge.svg)](https://docs.rs/memchain)

This crate provides an in-memory blockchain with Ethereum-like semantics.
Memchain is primarily useful for integration tests.
In fact, it can be compiled to Wasm using `cargo build --target wasm32-unknown-unknown` and called from JavaScript (in Node or the browser) via its [FFI bindings](https://github.com/oasislabs/oasis/blob/master/memchain/src/ffi.rs).
To build the bindings, you'll want to pass `--features ffi`.
