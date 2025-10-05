set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default: build

fmt:
    cargo fmt --all

lint:
    scripts/check-vendor.sh
    cargo clippy --all-targets --all-features -- -D warnings
    if [ -f deny.toml ]; then cargo deny check; else echo "skip cargo-deny (deny.toml not found)"; fi

build:
    scripts/check-vendor.sh
    cargo build --workspace

test:
    scripts/check-vendor.sh
    cargo test --workspace -- --nocapture

run-core:
    scripts/check-vendor.sh
    cargo run -p hauski-core

run-core-expose:
    scripts/check-vendor.sh
    HAUSKI_EXPOSE_CONFIG=true cargo run -p hauski-core

run-cli ARGS='':
    scripts/check-vendor.sh
    cargo run -p hauski-cli -- {{ARGS}}

vendor:
    mkdir -p vendor
    cargo vendor --locked --respect-source-config > /dev/null

vendor-archive:
    rm -f hauski-vendor-snapshot.tar.zst
    tar --zstd -cvf hauski-vendor-snapshot.tar.zst vendor
    du -h hauski-vendor-snapshot.tar.zst || true

# Python tooling via uv

py-init:
    uv sync --group dev --frozen

py-lint:
    uv run ruff check .

py-fmt:
    uv run ruff format .

py-test:
    if [ -d "tests" ]; then
        uv run pytest -q
    elif ls tests_*.py >/dev/null 2>&1; then
        uv run pytest -q
    else
        echo "No Python tests found – skipping."
    fi

py-docs-serve:
    uv run mkdocs serve -a 0.0.0.0:8000

py-docs-build:
    uv run mkdocs build --strict --clean

py-pre-commit:
    uv run pre-commit run --all-files
