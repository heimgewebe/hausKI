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
    cargo run -p hauski-cli -- serve

# Llama.cpp HTTP-Server lokal starten (einfacher Default)
# Aufruf: `just llama-server` oder `just llama-server MODEL=/opt/models/llama3.1-8b-q4.gguf PORT=8081`
llama-server MODEL="/opt/models/llama3.1-8b-q4.gguf" PORT="8081" HOST="127.0.0.1" BIN="./llama.cpp/server":
    {{BIN}} -m {{MODEL}} --host {{HOST}} --port {{PORT}}

# Demo-Call an /v1/chat (zeigt 200 mit Content, wenn chat_upstream_url gesetzt ist; sonst 501)
chat-demo TEXT="Hallo HausKI!":
    curl -s -X POST http://127.0.0.1:8080/v1/chat \
      -H 'Content-Type: application/json' \
      -d '{{"messages":[{"role":"user","content":"{{+TEXT+}}"}]}}' | jq

run-core-expose:
    scripts/check-vendor.sh
    HAUSKI_EXPOSE_CONFIG=true cargo run -p hauski-cli -- serve

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
    if [ -d "tests" ]; then uv run pytest -q; elif ls tests_*.py >/dev/null 2>&1; then uv run pytest -q; else echo "No Python tests found – skipping."; fi

py-docs-serve:
    uv run mkdocs serve -a 0.0.0.0:8000

py-docs-build:
    uv run mkdocs build --strict --clean

py-pre-commit:
    uv run pre-commit run --all-files

shadow:
    uv run uvicorn services.policy_shadow.app:app --reload --port 8085

# Quick vs. Full
test-quick:
    @echo "running quick tests…"
    @if command -v cargo >/dev/null; then cargo test -q; fi
    @if command -v uv >/dev/null; then uv run pytest -q || true; fi
    @if command -v npm >/dev/null; then npm test -s || true; fi

test-full:
    @echo "running full test suite…"
    @if command -v cargo >/dev/null; then cargo test -q; fi
    @if command -v uv >/dev/null; then uv run pytest -q || true; fi
    @if command -v npm >/dev/null; then npm test -s || true; fi

# Codex Runs
codex-doctor:
    @echo "🔎 Checking codex availability…"
    @if command -v codex >/dev/null; then echo "✅ codex in PATH"; \
    else echo "ℹ️  using npx @openai/codex@1.0.0"; fi

codex-bugfix:
    bash scripts/hauski-codex.sh . scripts/codex-prompts/bugfix.md scripts/policies/codex.policy.yml

codex-testgap:
    bash scripts/hauski-codex.sh . scripts/codex-prompts/testgap.md scripts/policies/codex.policy.yml

codex-refactor:
    bash scripts/hauski-codex.sh . scripts/codex-prompts/refactor.md scripts/policies/codex.policy.yml

emit-event id='auto' kind='debug.test' payload='{"ok":true}':
    @echo "Emitting event: {{kind}}"
    if [ "{{id}}" = "auto" ]; then id_arg=""; else id_arg="{{id}}"; fi
    NODE_ID="$(hostname)" KIND="{{kind}}" PAYLOAD='{{payload}}' ./scripts/emit_event_line.sh ${id_arg}
