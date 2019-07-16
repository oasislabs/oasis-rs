# Blockchain FileSystem (BCFS)

[![Latest Version](https://img.shields.io/crates/v/bcfs.svg)](https://crates.io/crates/bcfs)
[![docs](https://docs.rs/bcfs/badge.svg)](https://docs.rs/bcfs)

This crate provides a blockchain filesystem for use in a WASI Wasm runtime.
The implementation is based on the [Blockchain WASI proposal](https://github.com/oasislabs/rfcs/pull/1) ([link to high-level blog post](https://medium.com/oasislabs/blockchain-flavored-wasi-50e3612b8eba)).

You can find examples of using BCFS in [`src/lib/tests.rs`](https://github.com/oasislabs/oasis/blob/master/bcfs/src/tests.rs#L79).

BCFS can be compiled to Wasm so that it can be used in integration tests.
Just build using `cargo build --target wasm32-unknown-unknown`.
BCFS exposes [FFI bindings](https://github.com/oasislabs/oasis/blob/master/bcfs/src/ffi.rs) so that it can be called from, say, JavaScript in the browser.
To build the bindings, you'll want to pass `--features ffi`.
