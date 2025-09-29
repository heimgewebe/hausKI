#!/usr/bin/env bash
set -euo pipefail

sudo apt-get update -y
# Rust toolchain (if feature didn't handle fully)
if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi
rustup component add clippy rustfmt

# Shell / QA
sudo apt-get install -y shellcheck shfmt jq moreutils
# Node-based linters optional
if command -v npm >/dev/null 2>&1; then
  npm i -g markdownlint-cli@0.41.0
fi

echo "Dev tools ready: $(rustc --version); clippy; rustfmt; shellcheck; shfmt"
