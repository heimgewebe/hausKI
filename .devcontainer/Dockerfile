FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential cmake ninja-build pkg-config git curl unzip wget ca-certificates \
    clang lld llvm-dev libclang-dev libssl-dev \
    libsndfile1-dev libasound2-dev libopenblas-dev \
    ffmpeg sox sqlite3 libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

USER vscode
SHELL ["/bin/bash", "-lc"]

# Rust toolchain (respektiert rust-toolchain.toml im Repo)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal && \
    rustup component add rustfmt clippy

# hilfreiche Cargo-Tools
RUN cargo install cargo-deny cargo-edit just
WORKDIR /workspaces/hauski