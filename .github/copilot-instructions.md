# Copilot Instructions for HausKI

Diese Datei richtet sich an GitHub Copilot / Copilot Agents und beschreibt, wie in diesem Repository gearbeitet werden soll.

Ziel: Copilot soll HausKI als Rust-zentrierten, lokalen KI-Orchestrator verstehen und Änderungen vorschlagen, die

- zum bestehenden Architekturmodell passen,
- die Toolchain nicht brechen,
- die Definition-of-Done respektieren,
- und in kleinen, gut überprüfbaren Schritten erfolgen.

---

## 1. Projektüberblick

- **Projekt:** HausKI – lokaler KI-Orchestrator für Pop!_OS-Workstations mit NVIDIA-RTX-GPU
- **Ansatz:** Offline-first, datensparsam, keine versteckten Netzwerkabhängigkeiten
- **Architektur:** Monorepo mit klar getrennten Bereichen:
  - CLI (User-Einstieg)
  - Core (axum-Server, Services)
  - Policies (Routing, Limits)
  - Models/Configs (Modell- und Feature-Flags)
  - Python-Services (Adapter, Shadow-Logik)

**Wichtige Annahme:**
Pop!_OS mit NVIDIA-RTX ist der Referenz-Stack. Anpassungen dürfen andere Plattformen (Termux, WSL, Codespaces) nicht „brechen“, sondern müssen optional bleiben.

---

## 2. Sprache, Stil und Commits

### Sprache

- **Deutsch** für:
  - Dokumentation (`README`, `docs/`, Runbooks)
  - Commit-Nachrichten
  - CLI-Hilfetexte
- **Englisch** für:
  - Code-Kommentare
  - Log-Meldungen
  - Fehlermeldungen im Code

Keine Gender-Sonderzeichen (`*`, `:`, `·`, `_`, Binnen-I). Neutrale Formulierungen verwenden.

### Commit-Konventionen

Conventional Commits mit kurzen, präzisen Messages:

Beispiele:

- `feat(cli): add conversation history export`
- `fix(core): handle missing policy in request path`
- `docs: document GPU memory limits`
- `refactor(indexd): simplify query builder`
- `chore(core): update dependencies`

Commits sollen kleine, logische Einheiten bilden:

- ein neues CLI-Feature,
- eine isolierte Bugfix,
- ein kleiner Refactor,
- eine gezielte Dokumentationsänderung.

---

## 3. Projektstruktur (für Copilot wichtig)

Das Repository ist ein Cargo-Workspace. Wichtige Pfade:

- `crates/cli`
  - Kommandozeilen-Einstiegspunkt (clap)
  - Verantwortlich für Argument-Parsing, Konfiguration, Starten des Core-Servers

- `crates/core`
  - axum-Server, zentrale Services, Auth, HTTP-API
  - Policies werden hier konsumiert, nicht definiert

- `crates/embeddings`
  - Logik zur Erzeugung von Text-Embeddings

- `crates/indexd`
  - Indizierung und Suche (SQLite + tantivy)

- `crates/memory`
  - Persistenter Key-Value-Store (SQLite)

- `crates/policy`
  - Policy-Datenstrukturen und Evaluierungslogik

- `crates/policy_api`
  - API-Schicht zur Policy-Engine (Requests/Responses, Contracts)

- `configs/`
  - Konfigurationsdateien (z. B. `models.yml`, `flags.yaml`)

- `policies/`
  - Routing, Ratenbegrenzungen, Sicherheits- und Ressourcenregeln

- `services/`
  - Python-Dienste (z. B. `policy_shadow`, Hilfsmodule)

- `docs/`
  - Runbooks, Architekturübersichten, Troubleshooting

---

## 4. Entwicklungs- und Tooling-Kontext

HausKI wird in einem Devcontainer und lokal entwickelt.

### Devcontainer

- `.devcontainer/` enthält die Referenzumgebung:
  - Rust-Toolchain
  - CUDA-Basis
  - `cargo-deny`
  - `just`
  - Vale (Prose-Linting)

Copilot soll davon ausgehen, dass diese Tools verfügbar sind, wenn im Container gearbeitet wird.

### Profile

- `.wgx/profile.yml` – kanonisches Profil
- `.wgx/profile.local.yml` – lokale Anpassungen

Änderungen im Tooling sollten dieses Profil-System respektieren und nicht stillschweigend ignorieren.

---

## 5. Build, Lint & Test

Vorbereitete Standardbefehle:

```bash
# Rust
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo deny check
cargo test --workspace -- --nocapture

# Prose
vale .

Alternativ über just:

just fmt
just lint
just build
just test

Python-Tooling

Einige Dienste liegen in services/ und nutzen Python:

just py-init    # uv sync --extra dev --locked --frozen
just py-lint    # Ruff
just py-fmt     # Ruff format
just py-test    # pytest

Wichtig:
Rust- und Python-Teile müssen jeweils ihre Tests bestehen. Copilot soll neue Funktionen so vorschlagen, dass bestehende just-Tasks nicht brechen.

---

## 6. Coding Conventions (Rust und Shell)

### Rust

- Formatierung immer mit cargo fmt.
- Benennung:
  - snake_case für Variablen und Funktionen
  - PascalCase für Typen, Structs, Enums
- Fehlerbehandlung:
  - thiserror für eigene Fehler-Typen
  - anyhow für flexible Fehlerweitergabe
- Öffentliche Funktionen und Typen dokumentieren (///-Kommentare).
- In performancekritischen Pfaden lieber explizit optimieren als „magische“ Abstraktionen hinzufügen.

### Shell-Skripte

- Immer mit set -euo pipefail beginnen.
- Skripte sollten:
  - Eingaben validieren,
  - Pfade und Umgebungsvariablen klar dokumentieren,
  - keine unkontrollierten Netzwerkzugriffe ausführen.

### CLI-Kommandos

- Jedes Kommando benötigt -h | --help mit sinnvollen Beschreibungen.
- Beispiele in der Hilfe sind erwünscht (Deutsch).
- Fehlerausgaben bei falscher Benutzung: klar, kurz, informativ.

---

## 7. Sicherheitsrichtlinien

Copilot soll diese Regeln bei Vorschlägen berücksichtigen:

- Keine stillen Fehler:
  - Fehler nicht „verschlucken“, sondern mit Kontext loggen.
  - unwrap() und expect() in nicht-testendem Code vermeiden, außer in sehr eng kontrollierten Pfaden.
- Keine unkontrollierten Netzwerkzugriffe:
  - HausKI ist offline-first.
  - Neue HTTP-Aufrufe, Cloud-Abhängigkeiten oder Telemetrie sind nicht erwünscht, außer explizit in der Architektur vorgesehen.
- Plattform:
  - Pop!_OS mit CUDA/NVIDIA ist Referenz.
  - Termux, WSL, Codespaces dürfen nicht durch harte Annahmen über Pfade oder Distributionen unbenutzbar werden.
  - Linux-Distro-spezifische Flags oder Pakete nur mit sinnvollem Fallback.
- Performancekritische Pfade:
  - In Rust implementieren, nicht in Python.
  - Riskante Adapter oder experimentelle Logik wenn möglich in Wasm isolieren.

---

## 8. Definition of Done (DoD)

Eine Änderung gilt als fertig, wenn:

1. CI grün ist:
   - cargo fmt
   - cargo clippy (ohne Warnungen)
   - cargo test (workspaceweit)
   - cargo deny
   - vale (für relevante Dokumentation)
2. Für CLI-Kommandos zusätzlich:
   - Hilfetext aktualisiert
   - Minimaltests vorhanden (Unit- oder Integrationstests)
   - Dokumentation (README oder docs/) angepasst, falls Verhalten geändert wurde
3. Policies und Modelle:
   - Änderungen in configs/ und policies/ sind dokumentiert.
   - Auswirkungen auf Limits, Routing oder Ressourcenverbrauch sind nachvollziehbar.
4. GPU-relevante Änderungen:
   - Auswirkungen auf GPU-Speicher, Thermik und Limits sind kurz dokumentiert (z. B. in docs/ oder entsprechendem Runbook).

---

## 9. Spezifische Anweisungen an Copilot

Wenn du (Copilot / Agent) Code vorschlägst:

1. Kleinschrittig arbeiten
   - Kleine PRs, wenig Dateien auf einmal.
   - Einen klaren Fokus pro Änderung (z. B. nur ein neues CLI-Flag, nur eine Policy-Anpassung).
2. Bestehende Strukturen bevorzugen
   - Keine neuen Frameworks oder großen Abhängigkeiten vorschlagen.
   - Nur auf bereits verwendete Bibliotheken zurückgreifen (z. B. axum, thiserror, anyhow, tantivy).
3. Kein spontanes API-Design ohne Kontext
   - Bei neuen Endpoints: an vorhandene Muster in crates/core und crates/policy_api anlehnen.
   - Bestehende DTOs, Enums und Error-Typen wiederverwenden.
4. Policies sehr vorsichtig bearbeiten
   - policies/ und configs/models.yml nur ändern, wenn der Zweck klar ist.
   - Keine „sicherheitshalber“-Änderungen an Limits oder Berechtigungen.
5. Python-Services
   - Dienste in services/ sollen klar abgegrenzt bleiben.
   - Kein Vermischen von Rust- und Python-Pflichten (z. B. keine Businesslogik doppelt implementieren).
6. Vendoring / Offline-Builds
   - Vor neuen Abhängigkeiten prüfen, ob das Vendor-Konzept (scripts/check-vendor.sh) betroffen wäre.
   - Möglichst keine Abhängigkeit hinzufügen, die schwer zu vendoren ist.

---

## 10. Typische Aufgabenbeispiele für Copilot

Copilot darf u. a. helfen bei:

- Neues CLI-Subkommando anlegen
  - Ort: crates/cli
  - Beispiel: Kommando für Status-Abfrage oder Export von Logs
  - Erwartet: --help-Text, Tests, Dokumentation
- Neuen HTTP-Endpoint im Core ergänzen
  - Ort: crates/core
  - Ablauf:
    1. Route registrieren
    2. Handler implementieren
    3. Fehler- und Auth-Handling gemäß vorhandenen Mustern
    4. Tests schreiben
- Kleine Refactors
  - Duplizierten Code reduzieren
  - Fehlerbehandlung vereinheitlichen
  - Logging verbessern (ohne Logspam)
- Dokumentation ergänzen
  - Beispiele in README oder docs/ aktualisieren
  - Runbooks erweitern, falls neue Betriebsmodi hinzukommen

---

## 11. Weiterführende Dokumentation

- CONTRIBUTING.md – detaillierte Beitragsrichtlinien
- README.md – Projektübersicht und Schnellstart
- docs/ – Runbooks, Architektur, Betriebsanleitungen

Wenn unklar ist, wie eine Änderung eingebettet werden soll, bevorzugt in diesen Dokumenten nach Mustern suchen, statt eigene Strukturen zu erfinden.
