#!/usr/bin/env bash
set -euo pipefail

tmp_config=""
tmp_vendor=""
config_path=""
config_backup=""

cleanup() {
  if [[ -n "$tmp_config" && -f "$tmp_config" ]]; then
    rm -f "$tmp_config"
    tmp_config=""
  fi
  if [[ -n "$tmp_vendor" && -d "$tmp_vendor" ]]; then
    rm -rf "$tmp_vendor"
    tmp_vendor=""
  fi
  if [[ -n "$config_backup" && -f "$config_backup" ]]; then
    mv "$config_backup" "$config_path"
    config_backup=""
  fi
}

trap cleanup EXIT

log() { printf "%s\n" "$*" >&2; }
die() {
  log "ERR: $*"
  exit 1
}
need() { command -v "$1" >/dev/null 2>&1 || die "Fehlt: $1"; }

need cargo

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo ".")"
cd "$ROOT"
config_path="$ROOT/.cargo/config.toml"

# --- Optionen ---
NO_NETWORK="${NO_NETWORK:-0}"             # 1 = zwingend offline bauen (CI nach Vendoring)
NEUTRALIZE_PROXY="${NEUTRALIZE_PROXY:-1}" # 1 = Proxy-Variablen temporär leeren beim Vendoring

# --- Proxy ggf. neutralisieren, nur für den Vendoring-Teil ---
orig_http_proxy="${http_proxy:-}"
orig_https_proxy="${https_proxy:-}"
orig_HTTP_PROXY="${HTTP_PROXY:-}"
orig_HTTPS_PROXY="${HTTPS_PROXY:-}"
restore_proxy() {
  export http_proxy="${orig_http_proxy}"
  export https_proxy="${orig_https_proxy}"
  export HTTP_PROXY="${orig_HTTP_PROXY}"
  export HTTPS_PROXY="${orig_HTTPS_PROXY}"
}
neutralize_proxy() {
  export http_proxy=""
  export https_proxy=""
  export HTTP_PROXY=""
  export HTTPS_PROXY=""
}

# --- Minimaler Sanity-Check: Lock + vendor Snapshot Zustand ---
has_lock() { [[ -f Cargo.lock ]]; }
has_vendor() {
  [[ -d vendor ]] || return 1
  [[ -f vendor/config.toml ]] && return 0
  [[ -d vendor/registry ]] && return 0
  compgen -G "vendor/*" >/dev/null 2>&1
}

missing_axum() {
  shopt -s nullglob
  shopt -s globstar
  for path in vendor/**/axum-*; do
    [[ -d "$path" ]] && return 1
  done
  return 0
}

if [[ "${NO_NETWORK}" == "1" ]]; then
  log "🌙 NO_NETWORK=1 → erwarte vollständigen vendor/ Snapshot. Kein Online-Zugriff."
  has_lock || die "Cargo.lock fehlt im Offline-Modus."
  has_vendor || die "vendor/ fehlt im Offline-Modus."
  if missing_axum; then
    die "axum ist im vendor/ nicht auffindbar. Snapshot ist unvollständig."
  fi
  log "✅ Offline-Check ok."
  exit 0
fi

# --- Online (oder zumindest mit Netzwerk) Vendoring vorbereiten ---
if [[ ! -f Cargo.lock ]]; then
  log "🔧 Erzeuge Cargo.lock (generate-lockfile)…"
  if [[ "${NEUTRALIZE_PROXY}" == "1" ]]; then neutralize_proxy; fi
  cargo generate-lockfile
  if [[ "${NEUTRALIZE_PROXY}" == "1" ]]; then restore_proxy; fi
fi

log "🔧 Erzeuge/aktualisiere vendor-Snapshot (locked, versioned-dirs)…"
if tmp_vendor=$(mktemp -d vendor.tmp.XXXXXX); then
  args=(vendor --locked --versioned-dirs "$tmp_vendor")
else
  die "mktemp für vendor.tmp fehlgeschlagen"
fi
if [[ "${NEUTRALIZE_PROXY}" == "1" ]]; then neutralize_proxy; fi
if [[ -f "$config_path" ]]; then
  config_backup="${config_path}.ensure-vendor.bak"
  mv "$config_path" "$config_backup"
fi
if tmp_config=$(mktemp); then
  cat >"$tmp_config" <<'CFG'
[net]
git-fetch-with-cli = true
retry = 1

[registries.crates-io]
protocol = "sparse"
CFG
  CARGO_SOURCE_CRATES_IO_REPLACE_WITH="" CARGO_CONFIG="$tmp_config" cargo "${args[@]}"
  rm -f "$tmp_config"
  tmp_config=""
else
  CARGO_SOURCE_CRATES_IO_REPLACE_WITH="" cargo "${args[@]}"
fi
rm -rf vendor
mv "$tmp_vendor" vendor
tmp_vendor=""
if [[ -n "$config_backup" ]]; then
  mv "$config_backup" "$config_path"
  config_backup=""
fi
if [[ "${NEUTRALIZE_PROXY}" == "1" ]]; then restore_proxy; fi

# Diagnose: ist axum nun da?
if missing_axum; then
  log "⚠️  Hinweis: axum wurde im vendor/ nicht gefunden."
  log "    Prüfe, ob axum wirklich eine direkte oder indirekte Abhängigkeit ist:"
  log "      cargo tree -e features | grep -i axum || true"
  cargo tree -e features | grep -i axum || true
  die "Vendoring abgeschlossen, aber axum fehlt → Abhängigkeitsauflösung/Lock prüfen."
fi

log "✅ vendor/ Snapshot steht."
