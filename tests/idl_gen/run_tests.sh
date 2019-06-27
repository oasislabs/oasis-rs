#!/bin/bash
cargo build --target wasm32-wasi
cargo test
