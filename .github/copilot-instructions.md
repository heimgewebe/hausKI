# Copilot Instructions for HausKI

Diese Dokumentation richtet sich an GitHub Copilot (Coding Agent) und erklärt die Konventionen, Struktur und Arbeitsweise des HausKI-Repositorys.

## Projektübersicht

HausKI ist ein Rust-zentrierter, lokaler KI-Orchestrator für Pop!_OS-Workstations mit NVIDIA-RTX-GPU. Das Projekt verfolgt einen Offline-First-Ansatz und ist als Monorepo mit klaren Schnittstellen (CLI, Core, Policies, Modelle) organisiert.

## Sprache & Dokumentation

- **Deutsch** für Dokumentation, Commit-Nachrichten und Hilfetexte
- **Englisch** für Code-Kommentare und Log-Meldungen
- Keine Gender-Sonderzeichen (`*`, `:`, `·`, `_`, Binnen-I); nutze neutrale Formulierungen
- Conventional-Commit-Präfixe verwenden: `feat:`, `fix:`, `docs:`, `refactor:`, `chore(core):`

## Entwicklungsumgebung

- Devcontainer (`.devcontainer/`) mit Rust, CUDA-Basis, `cargo-deny`, `just` und Vale
- Lokale Entwicklung erfordert Rust, CUDA-Treiber, Vale und `cargo-deny`
- Profile in `.wgx/profile.yml` und `.wgx/profile.local.yml`

## Build, Lint & Test

Verwende diese Befehle zum Prüfen der Codequalität:

```bash
# Formatierung
cargo fmt --all

# Lints
cargo clippy --all-targets --all-features -- -D warnings
cargo deny check

# Tests
cargo test --workspace -- --nocapture

# Prose-Linting
vale .
```

Alternativ über die `justfile`:

```bash
just fmt
just lint
just build
just test
```

Für Python-Tooling:

```bash
just py-init    # uv sync --extra dev --locked --frozen
just py-lint    # Ruff
just py-fmt     # Ruff Format
just py-test    # pytest
```

## Projektstruktur

Das Repository ist als Cargo-Workspace organisiert:

- `crates/cli` – Kommandozeilen-Einstieg (clap)
- `crates/core` – axum-Server, Policies, Auth, zentrale Services
- `crates/embeddings` – Vektor-Embeddings aus Textdaten
- `crates/indexd` – SQLite + tantivy für Indizierung/Suche
- `crates/memory` – Persistenter Key-Value-Store (SQLite)
- `crates/policy` – Policy-Datenstrukturen und Logik
- `crates/policy_api` – API für Policy-Engine
- `configs/` – Konfigurationsdateien (models.yml, flags.yaml)
- `policies/` – Routing- und Limit-Definitionen
- `services/` – Python-Dienste (z. B. policy_shadow)
- `docs/` – Dokumentation und Runbooks

## Coding Conventions

- **Formatierung**: Code mit `cargo fmt` formatieren
- **Benennung**: `snake_case` für Variablen/Funktionen, `PascalCase` für Typen
- **Fehlerbehandlung**: `thiserror` und `anyhow` verwenden
- **Dokumentation**: Alle öffentlichen Funktionen und Typen dokumentieren
- **Shell-Skripte**: Mit `set -euo pipefail` starten
- **CLI-Kommandos**: Müssen `-h|--help` anbieten

## Sicherheitsrichtlinien

- Keine stillen Fehler oder unkontrollierte Netzwerkzugriffe
- Pop!_OS ist Referenz-Stack; Termux/WSL/Codespaces dürfen nicht brechen
- Keine Linux-Distro-spezifischen Flags ohne Absicherung
- Performance-kritische Pfade in Rust; riskante Adapter isoliert in Wasm

## Definition of Done

Änderungen gelten als fertig, wenn:

- CI grün: `cargo fmt`, `cargo clippy`, `cargo test`, `cargo deny`, Vale
- Für CLI-Kommandos: Hilfetext, Tests und Dokumentation vorhanden
- Policies/Modelle dokumentiert und in `configs/`/`policies/` gepflegt
- GPU-relevante Änderungen dokumentiert (Thermik, Speicher, Limits)

## Wichtige Hinweise

- Änderungen sollen klein, überprüfbar und reproduzierbar bleiben
- Commits klein halten und logisch gruppieren
- PR-Beschreibung: Fokus, Motivation und „Wie getestet" angeben
- Vendor-Verzeichnis prüfen vor Offline-Builds (`scripts/check-vendor.sh`)

## Weiterführende Dokumentation

- [CONTRIBUTING.md](../CONTRIBUTING.md) – Detaillierte Beitragsrichtlinien
- [README.md](../README.md) – Projektübersicht und Schnellstart
- [docs/](../docs/) – Runbooks und technische Dokumentation
