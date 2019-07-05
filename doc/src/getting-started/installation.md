# Installation

Mantle is designed around the Rust programming language.
Accordingly, you will need the Rust toolchain and the Mantle build tool.

The canonical way to get Rust is to use [`rustup`](https://rustup.rs).
Mantle is currently tested to works with the `nightly-2019-07-03` toolchain, so if you're just starting with Rust, you'll want to use that.
Advanced Rustaceans may try newer nightlies at their own risk.

Install Rustup using the following curl-pipe-sh (or download and run the commands manually, if that's more your style).
If you already have Rustup, just `rustup default nightly-2019-07-03`

```console
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- --default-toolchain nightly-2019-07-03
```

You will also need support for the `wasm32-wasi` target to produce binaries that can be deployed to the blockchain.

```console
rustup target add wasm32-wasi
```

Finally, using Cargo, the Rust package manager, install the Mantle build tool.

```console
cargo install mantle-build
```

## Optional Tools

The following tools are not required for developing Mantle services, but they'll make your life much easier.

* `rustup component add rustfmt`: [Rustfmt](https://github.com/rust-lang/rustfmt) formats Rust code according to style guidelines. It allows you to write arbitrarily sloppy Rust code and still get something that looks nice. If you also install an editor plugin, it helps you catch syntax errors before running `cargo build`.
* `rustup component add clippy`: [Clippy](https://github.com/rust-lang/rust-clippy#usage) is a linter for Rust code. Clippy catches "semantic" bugs like useless closures and potentially slow operations.
* `cargo install twiggy`: [Twiggy](https://rustwasm.github.io/twiggy/) is a Wasm code size profiler that you can use to optimize your service binaries before you deploy them to the blockchain.
* [The WebAssembly Binary Toolkit (WABT)](https://github.com/WebAssembly/wabt): WABT is a collection of useful low-level tools for debugging and manipulating Wasm binaries.
