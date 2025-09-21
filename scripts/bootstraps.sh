#!/usr/bin/env bash
set -euo pipefail

echo "▶ HausKI bootstrap…"

# 0) Git vorhanden? Hooks nur setzen, wenn .git existiert
if [[ -d .git ]]; then
  mkdir -p .git/hooks
  cat > .git/hooks/pre-commit <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
echo "pre-commit: fmt + clippy"
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
EOF
  chmod +x .git/hooks/pre-commit || true
fi

# 1) Rust toolchain Sichtprüfung
rustc --version || true
cargo --version || true

# 2) lokale Default-Env
if [[ ! -f .env ]]; then
  cat > .env <<'ENV'
RUST_LOG=info,hauski=debug
HAUSKI_MODELS=./configs/models.yml
HAUSKI_LIMITS=./policies/limits.yaml
ENV
fi

# 3) GPU Smoke-Test im Container (optional – nicht fatal)
if command -v nvidia-smi >/dev/null 2>&1; then
  echo "GPU visible inside container:"
  nvidia-smi || true
else
  echo "Hinweis: nvidia-smi im Container nicht gefunden (ok, wenn Host-Runtime mapped)."
fi

echo "✔ bootstrap done."