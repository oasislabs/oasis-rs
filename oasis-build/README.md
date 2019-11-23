# oasis-build

[![Latest Version](https://img.shields.io/crates/v/oasis-build.svg)](https://crates.io/crates/oasis-build)
[![docs](https://docs.rs/oasis-build/badge.svg)](https://docs.rs/oasis-build)

oasis-build is a compiler plugin that adds boilerplate code for calling and deploying oasis services and generates
a JSON (or protobuf) description of the RPC interface.

You can use `oasis-build` directly by setting `RUSTC_WRAPPER=oasis-build` or, more conveniently, using the [Oasis CLI](https://github.com/oasislabs/oasis-cli/) (included with the default toolchain).
