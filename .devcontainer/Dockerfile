# HausKI base image
FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

ARG CARGO_DENY_VERSION=0.14.1

# Basistools
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates curl git build-essential pkg-config xz-utils \
 && rm -rf /var/lib/apt/lists/*

# cargo-deny wird systemweit installiert, damit der Befehl in allen Terminals verfügbar ist
RUN curl -sSLf "https://github.com/EmbarkStudios/cargo-deny/releases/download/${CARGO_DENY_VERSION}/cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl.tar.xz" \
    | tar -xJ --strip-components=1 -C /usr/local/bin "cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl/cargo-deny"
