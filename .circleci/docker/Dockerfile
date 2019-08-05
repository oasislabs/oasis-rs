FROM ubuntu:18.04

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get -qq install \
    curl \
    git \
    build-essential \
    libssl-dev \
    pkg-config \
    python3 \
    python3-pip

RUN pip3 install boto3 pylint yapf pytest

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly-2019-08-01 && \
    . /root/.cargo/env && \
    rustup component add rustfmt && \
    rustup target add wasm32-wasi wasm32-unknown-unknown && \
    echo "source ~/.cargo/env" >> ~/.bashrc
