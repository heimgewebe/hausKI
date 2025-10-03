# HausKI base image
FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

ARG CARGO_DENY_VERSION=0.14.1

# Basistools
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates curl git build-essential pkg-config xz-utils \
 && rm -rf /var/lib/apt/lists/*

# cargo-deny wird systemweit installiert, damit der Befehl in allen Terminals verfügbar ist
# Download cargo-deny archive and its checksum, verify integrity, then extract
RUN set -e; \
    ARCHIVE="cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl.tar.xz"; \
    CHECKSUMS_URL="https://github.com/EmbarkStudios/cargo-deny/releases/download/${CARGO_DENY_VERSION}/cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl.tar.xz.sha256"; \
    curl -sSLf -o "$ARCHIVE" "https://github.com/EmbarkStudios/cargo-deny/releases/download/${CARGO_DENY_VERSION}/$ARCHIVE"; \
    curl -sSLf -o "$ARCHIVE.sha256" "$CHECKSUMS_URL"; \
    sha256sum -c "$ARCHIVE.sha256"; \
    tar -xJ --strip-components=1 -C /usr/local/bin "$ARCHIVE" "cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl/cargo-deny"; \
    rm "$ARCHIVE" "$ARCHIVE.sha256"
# Install uv (astral)
RUN curl -LsSf https://astral.sh/uv/install.sh | sh \
    && install -m 0755 /root/.cargo/bin/uv /usr/local/bin/uv

