# Beitrag zu HausKI

**Rahmen:** HausKI ist ein Rust-zentriertes KI-Orchestrator-Projekt mit Fokus auf Offline-Betrieb auf NVIDIA-RTX-Hardware. Ziel ist ein wartbares Monorepo mit klaren Schnittstellen (CLI, Core, Policies, Modelle). Änderungen sollen klein, überprüfbar und reproduzierbar bleiben.

## Grundregeln

- **Sprache:** Dokumentation, Commit-Nachrichten und Hilfetexte auf Deutsch verfassen. Code-Kommentare und Logs bleiben Englisch.
- **Portabilität:** Pop!_OS ist der Referenz-Stack, doch Termux/WSL/Codespaces dürfen nicht brechen. Keine Linux-Distro-spezifischen Flags ohne Absicherung.
- **Sicherheit:** Shell-Skripte laufen mit `set -euo pipefail`. Keine stillen Fehler oder unkontrollierte Netzwerkzugriffe.
- **Hilfen:** CLI-Kommandos müssen `-h|--help` anbieten und beschreiben Defaults, Flags sowie Konfigurationspfade.

## Epistemische Infrastruktur (Wahrheit & Vertrauen)

HausKI setzt auf **explizite, überprüfbare Wahrheit**.

- **Test-Artefakte:**
  Aussagen über die Stabilität des Systems sind nur gültig, wenn sie durch das kanonische Artefakt belegt sind.
  Erzeuge dieses mit: `scripts/generate-test-summary.sh`.
  Das Ergebnis liegt in `artifacts/test.summary.json`.
  > "If it is not in this artifact, it is not verified truth."

- **Code Safety:**
  Beachte [ADR-0003: Code Safety](docs/adr/ADR-0003__code-safety.md).
  Kein `unwrap()` in Produktionscode, explizite Fehlerbehandlung ist Pflicht.

- **Inkonsistenzen:**
  Bekannte Abweichungen zwischen Vision und Code sind in `docs/inconsistencies.md` dokumentiert und klassifiziert (z. B. als `accepted limitation`).

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

## Development Workflow

Um zur `hauski` beizutragen, folgen Sie bitte diesen Schritten:

1. **Forken Sie das Repository**: Erstellen Sie einen Fork des Haupt-Repositorys in Ihrem eigenen GitHub-Account.
2. **Klonen Sie den Fork**: Klonen Sie Ihren Fork auf Ihre lokale Maschine.
3. **Erstellen Sie einen Branch**: Erstellen Sie einen neuen Branch für Ihre Änderungen.
4. **Nehmen Sie Änderungen vor**: Implementieren Sie Ihre Änderungen und stellen Sie sicher, dass sie den Coding Style Guide einhalten.
5. **Führen Sie Tests durch**: Führen Sie die Tests aus, um sicherzustellen, dass Ihre Änderungen keine bestehenden Funktionalitäten beeinträchtigen.
6. **Committen Sie Ihre Änderungen**: Committen Sie Ihre Änderungen mit einer aussagekräftigen Commit-Nachricht.
7. **Pushen Sie Ihre Änderungen**: Pushen Sie Ihre Änderungen in Ihren Fork.
8. **Erstellen Sie einen Pull Request**: Erstellen Sie einen Pull Request vom Ihrem Branch in den `main`-Branch des Haupt-Repositorys.

## Coding Style Guide

- **Formatierung**: Der Code sollte mit `cargo fmt` formatiert werden.
- **Benennung**: Variablen und Funktionsnamen sollten `snake_case` sein. Typnamen sollten `PascalCase` sein.
- **Dokumentation**: Alle öffentlichen Funktionen und Typen sollten dokumentiert werden.
- **Fehlerbehandlung**: Verwenden Sie `thiserror` und `anyhow` zur Fehlerbehandlung.
