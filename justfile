set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default: build

fmt:
    cargo fmt --all

lint:
    cargo clippy --all-targets --all-features -- -D warnings
    cargo deny check

build:
    cargo build --workspace

test:
    cargo test --workspace -- --nocapture

run-core:
    cargo run -p hauski-core

run-cli ARGS='':
    cargo run -p hauski-cli -- {{ARGS}}
