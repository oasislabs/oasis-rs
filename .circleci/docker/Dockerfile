FROM ubuntu:18.04

RUN apt-get update && apt-get -qq install \
    curl \
    git \
    build-essential \
    libssl-dev \
    pkg-config

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly && \
    . /root/.cargo/env && \
    rustup component add rustfmt && \
    rustup target add wasm32-unknown-unknown && \
    rustup target add wasm32-wasi && \
    cargo install owasm-utils-cli --bin wasm-build && \
    echo "source ~/.cargo/env" >> ~/.bashrc
