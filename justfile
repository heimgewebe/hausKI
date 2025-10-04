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

run-core-expose:
	HAUSKI_EXPOSE_CONFIG=true cargo run -p hauski-core

run-cli ARGS='':
	cargo run -p hauski-cli -- {{ARGS}}

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
