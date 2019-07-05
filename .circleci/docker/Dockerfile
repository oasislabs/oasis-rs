FROM ubuntu:18.04

RUN apt-get update && apt-get -qq install \
    curl \
    git \
    build-essential \
    libssl-dev \
    pkg-config \
    awscli

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly && \
    . /root/.cargo/env && \
    rustup component add rustfmt clippy && \
    rustup target add wasm32-wasi wasm32-unknown-unknown && \
    echo "source ~/.cargo/env" >> ~/.bashrc
