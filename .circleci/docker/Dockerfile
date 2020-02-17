FROM ubuntu:18.04

ENV PATH "/opt/wasi-sdk/bin:/root/.local/bin:/root/.cargo/bin:$PATH"
ENV LD_LIBRARY_PATH "/root/.rustup/toolchains/nightly-2020-02-16-x86_64-unknown-linux-gnu/lib:$LD_LIBRARY_PATH"

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get -qq install \
    curl \
    git \
    build-essential \
    libssl-dev \
    pkg-config \
    # convenience
    python3 python3-pip

RUN pip3 install boto3 pylint yapf pytest pytest-xdist

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly-2020-02-16 && \
    . /root/.cargo/env && \
    rustup component add rustfmt clippy rustc-dev && \
    rustup target add wasm32-wasi wasm32-unknown-unknown

RUN curl -sSLf https://github.com/CraneStation/wasi-sdk/releases/download/wasi-sdk-8/wasi-sdk_8.0_amd64.deb -o wasi-sdk_8.0_amd64.deb && \
    dpkg -i wasi-sdk_8.0_amd64.deb
