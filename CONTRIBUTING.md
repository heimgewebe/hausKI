# Beitrag zu HausKI

**Rahmen:** HausKI ist ein Rust-zentriertes KI-Orchestrator-Projekt mit Fokus auf Offline-Betrieb auf NVIDIA-RTX-Hardware. Ziel ist ein wartbares Monorepo mit klaren Schnittstellen (CLI, Core, Policies, Modelle). Änderungen sollen klein, überprüfbar und reproduzierbar bleiben.

## Grundregeln

- **Sprache:** Dokumentation, Commit-Nachrichten und Hilfetexte auf Deutsch verfassen. Code-Kommentare und Logs bleiben Englisch.
- **Portabilität:** Pop!_OS ist der Referenz-Stack, doch Termux/WSL/Codespaces dürfen nicht brechen. Keine Linux-Distro-spezifischen Flags ohne Absicherung.
- **Sicherheit:** Shell-Skripte laufen mit `set -euo pipefail`. Keine stillen Fehler oder unkontrollierte Netzwerkzugriffe.
- **Hilfen:** CLI-Kommandos müssen `-h|--help` anbieten und beschreiben Defaults, Flags sowie Konfigurationspfade.

## Entwicklungsumgebung

- Nutze den Devcontainer (`.devcontainer/`). Er bringt `rustup`, CUDA-Basis, `cargo-deny`, `just` und Vale mit.
- Lokale Entwicklung außerhalb des Containers erfordert die manuelle Installation von Rust, CUDA-Treibern, Vale und `cargo-deny`.
- Halte `.wgx/profile.yml` und etwaige lokale Overrides aktuell (`.wgx/profile.local.yml`).

## Lint & Tests

- Formatierung: `cargo fmt --all`.
- Lints: `cargo clippy --all-targets --all-features -- -D warnings` und `cargo deny check`.
- Tests: `cargo test --workspace -- --nocapture`.
- Optional: Vale für Prosa (`vale .`) und `wgx validate --profile .wgx/profile.yml` für Task-Definitionen.

## Commits & PRs

- Conventional-Commit-Präfixe verwenden (`feat: …`, `fix: …`, `docs: …`, `refactor: …`, `chore(core): …`).
- Commits klein halten, Änderungen logisch gruppieren und auf Deutsch beschreiben.
- PR-Beschreibung: Fokus, Motivation und „Wie getestet“ angeben. Hinweise auf GPU-spezifische Pfade oder Policies nicht vergessen.

## Definition of Done

- CI grün: `cargo fmt`, `cargo clippy`, `cargo test`, `cargo deny`, Vale (für Docs) und optionale WGX-Smoke-Checks.
- Für neue oder geänderte CLI-Kommandos: Hilfetext, Tests (bats oder Rust) und aktualisierte Dokumentation.
- Policies und Modelle müssen dokumentiert sowie in `configs/` bzw. `policies/` gepflegt werden.
- GPU-relevante Änderungen dokumentieren (Thermik, Speicher, Limits) und ggf. in der Roadmap ergänzen.
