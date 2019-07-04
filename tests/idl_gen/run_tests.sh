#!/bin/bash
cargo build --target wasm32-wasi --bins && cargo test
