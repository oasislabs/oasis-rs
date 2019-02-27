# oasis-std

A crate of utilities for developing programs for the Oasis platform.

The `Xargo.toml` can be used to create a custom Rust `libstd` that has wasm syscalls enabled.
This allows using `println!` and `panic!` directly without creating custom extern fns.
Compile using `--target=wasm32-unknown-unknown` to use Rust impls for symbols like
`memcpy`; use `--target=wasm32-unknown-emscripten` to use platform-provided versions.

## Usage

1. Add `oasis-std = "0.1"` to your contract's Cargo.toml.
   Pass `features = ["platform-alloc"]` to use the Oasis platform allocator.
2. Copy `Xargo.toml` to your contract crate root
3. `xargo build --target=wasm32-unknown-unknown`
4. business as usual
