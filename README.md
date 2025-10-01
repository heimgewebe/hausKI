# HausKI — Rust-first, Offline-Default, GPU-aware

HausKI ist ein lokaler KI-Orchestrator für Pop!_OS-Workstations mit NVIDIA-RTX-GPU.

**Hauptmerkmale:**
- Hot-Paths laufen in Rust (axum/tokio).
- Inferenz erfolgt über llama.cpp (FFI).
- ASR und TTS nutzen whisper-rs bzw. piper-rs.
- Wissen wird über ein VectorStore-Trait (tantivy + HNSW, optional Qdrant) verwaltet.
- Netzwerkzugriffe folgen einem "deny by default"-Modell.
---

## Inhalt
- [Quickstart](#quickstart)
- [Entwicklung im Devcontainer](#entwicklung-im-devcontainer)
- [Build, Test & Run](#build-test--run)
- [Policies & Budgets](#policies--budgets)
- [Modelle & Speicherorte](#modelle--speicherorte)
- [Architektur & Verzeichnisse](#architektur--verzeichnisse)
- [Roadmap-Fokus](#roadmap-fokus)
- [Contribution & Qualität](#contribution--qualität)
- [Weiterführende Dokumente](#weiterführende-dokumente)

---

## Schnellstart

**Voraussetzungen lokal (Pop!_OS, Rust stable):**
```bash
rustc --version && cargo --version
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo build --workspace
cargo test --workspace -- --nocapture
```

**VS Code Devcontainer:**
1. Repository klonen und in VS Code öffnen.
2. "Reopen in Container" ausführen; das Post-Create-Skript setzt `pre-commit` auf und prüft GPU-Verfügbarkeit.
3. Danach genügen die Shortcuts aus der `justfile` (`just build`, `just test`, `just run-core`).

---

## wgx-Manifest & Aufgaben

- Das wgx-Manifest liegt unter `.wgx/profile.yml` und verlangt einen Router mit den Fähigkeiten `task-router`, `manifest-validate` und `json-output`.
- Tasks bestehen aus `cmd` + `args` und kommen ohne `eval`-Magie aus; zusätzliche Argumente werden sauber durchgereicht.
- Prüfe das Manifest vor Commits lokal mit `wgx validate --profile .wgx/profile.yml`.
- Die JSON-Ansicht der Tasks erhältst du über `wgx tasks --json`.
- Persönliche Overrides (Pfade, Log-Level, Tokens) gehören in `.wgx/profile.local.yml`; eine Vorlage findest du in `.wgx/profile.local.example.yml`. Die Datei ist git-ignored.
- Gemeinsame Umgebungsvariablen landen im Manifest unter `envDefaults`, individuelle Anpassungen im lokalen Profil via `envOverrides`. So bleibt die Naming-Convention repo-weit konsistent.

---

## Entwicklung im Devcontainer
- Basis-Tooling: Rust, Node, Python sowie optionale CUDA-Runtime.
- Empfohlene Extensions: rust-analyzer, direnv, Docker, GitLens, Markdown-Mermaid.
- Vorgefertigte Tasks: `cargo: fmt`, `cargo: clippy`, `cargo: build`, `cargo: test`.
- Just-Shortcuts: `just fmt`, `just lint`, `just build`, `just test`, `just run-core`.
- Netzwerkzugriffe von Cargo nutzen standardmäßig den Sparse-Index (`CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`), um strenge Proxies zu umgehen; `CARGO_NET_RETRY=5` sorgt für automatische Wiederholungen.
- `cargo-deny` ist im Container global installiert und steht ohne zusätzliche Schritte bereit.

---

## Build, Test & Ausführung

Den Core-Service lokal starten:
```bash
cargo run -p hauski-cli -- serve
# Alternative über das Justfile (ruft intern `cargo run -p hauski-core` auf)
just run-core
```

> **Hinweis:** Setze `HAUSKI_EXPOSE_CONFIG=true`, um die geschützten Routen unter `/config/*` bewusst freizugeben (nur für lokale Tests empfohlen).

### CORS & Frontend-Integration

- Standardmäßig akzeptiert der Core nur Browser-Anfragen vom Ursprung `http://127.0.0.1:8080`.
- Setze `HAUSKI_ALLOWED_ORIGIN=<https://dein-frontend.example>` im Environment, um einen anderen Origin explizit freizuschalten.
- Preflight-Requests (`OPTIONS`) werden nur beantwortet, wenn der angefragte Origin erlaubt ist; alle anderen Ursprünge erhalten `403 Forbidden`.
- Die Antwort-Header enthalten `Access-Control-Allow-Origin` und `Vary: Origin`, sobald der Request vom freigegebenen Ursprung kommt.

### Docker-Compose-Stack (Profil `core`)

```bash
# Build & Start (detached)
docker compose -f infra/compose/compose.core.yml --profile core up --build -d

# Logs verfolgen
docker compose -f infra/compose/compose.core.yml logs -f api

# Optionaler Health- und Readiness-Check (falls implementiert)
curl http://localhost:${HAUSKI_API_PORT:-8080}/health
curl http://localhost:${HAUSKI_API_PORT:-8080}/ready

# Stoppen und Ressourcen freigeben
docker compose -f infra/compose/compose.core.yml --profile core down
```

Verfügbare bzw. geplante API-Endpunkte:
- `GET /health` → "ok"
- `GET /metrics`
- *(geplant)* OpenAI-kompatible Routen (`/v1/chat/completions`, `/v1/embeddings`)
- *(geplant)* Spezialendpunkte: `/asr/transcribe`, `/audio/profile`, `/obsidian/canvas/suggest`

### Observability & Metriken

Die API exportiert Prometheus-kompatible Kennzahlen unter `/metrics`:

- `http_requests_total{method,path,status}` zählt eingehende HTTP-Requests pro Methode, Route und Statuscode.
- `http_request_duration_seconds{method,path}` erfasst Latenzen als Histogramm mit den Standard-Buckets `0.005s` bis `1s`.

Beispielabfragen für Dashboards oder die Prometheus-Konsole:

- Erfolgs- vs. Fehlerraten:
  ```promql
  sum by (status) (rate(http_requests_total[5m]))
  ```
- 95%-Perzentil der Request-Latenz je Route:
  ```promql
  histogram_quantile(0.95, sum by (le, method, path) (rate(http_request_duration_seconds_bucket[5m])))
  ```

---

## Policies & Budgets
- Laufzeit- und Thermik-Grenzen liegen in `policies/limits.yaml` (z. B. `latency.llm_p95_ms`, `thermal.gpu_max_c`).
- Kann die Datei nicht gelesen werden, nutzt der Core sichere Defaults (LLM p95 = 400 ms, Index p95 = 60 ms, GPU-Max = 80 °C, dGPU-Power = 220 W, ASR WER = 10 %). So bleibt der Dienst lauffähig, selbst wenn Policies fehlen.
- Netzwerk-Routing folgt einem Deny-by-default-Ansatz; Whitelists werden perspektivisch über `policies/routing.yaml` gepflegt.
- CI verknüpft Formatierung, Lints (`cargo-deny`) und Tests; Budget-Checks für p95-Latenzen sind vorgesehen.

---

## Modelle & Speicherorte
- Modellkatalog in `configs/models.yml` mit ID, Pfad, VRAM-Profil und Canary-Flag.
- Beispielkonfiguration:

```yaml
models:
  - id: llama3.1-8b-q4
    path: /opt/models/llama3.1-8b-q4.gguf
    vram_min_gb: 6
    canary: false
  - id: whisper-medium
    path: /opt/models/whisper-medium.bin
    vram_min_gb: 4
    canary: true
```

- Geplante CLI-Kommandos (Preview):

> **Hinweis:** Die folgenden Kommandos sind geplante Beispiele und noch nicht implementiert. Platzhalter wie `<model-id>`, `<input-file>`, `<output-file>`, `<profile-name>` zeigen die erwartete Syntax.

```bash
hauski models pull <model-id>
hauski asr transcribe <input-file> --model <model-name> --out <output-file>
hauski audio profile set <profile-name>
```

---

## Architektur & Verzeichnisse
- `crates/core` – axum-Server, Policies, Auth, zentrale Services
- `crates/cli` – Kommandozeilen-Einstieg (clap)
- Geplante Erweiterungen: `indexd/` (SQLite + VectorStore), `llm/`, `asr/`, `tts/`, `audio/`, `memory/`, `commentary/`, `bridge/`, `observability/`, `security/`, `adapters/*` (Wasm)
- Grundsatz: Performance-kritische Pfade in Rust; riskante Adapter laufen isoliert in Wasm (wasmtime, systemd-cgroups/Namespaces).

---

## Roadmap-Fokus
- **P1 (aktiv):** Core + LLM + ASR + TTS, Budget-Guards, Basis-TUI, No-Leak-Guard.
- **P2:** Memory-Schichten, Commentary-Modul, Obsidian-Integration, Mail/Matrix, GPU-Power-Caps.
- **P3:** Bridge zu NATS/JetStream, optionale Qdrant-Anbindung, CI/CD-Kurator, Luthier-Agent, Wake-Word v2.

---

## Contribution & Qualität
- CI-Gates: `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, `cargo-deny`.
- `scripts/bootstrap.sh` richtet `pre-commit` ein und erzwingt Format/Lint vor Commits.
- Lizenzrichtlinien liegen in `deny.toml`.

### Cargo.lock-Workflow
- `Cargo.lock` gilt als Build-Artefakt: Patches und PRs sollten die Datei zunächst ausschließen und sie erst nach einem lokalen `cargo update` committen.
- Das Skript [`scripts/git-apply-nolock.sh`](scripts/git-apply-nolock.sh) nimmt ein Patch-File entgegen, wendet es ohne `Cargo.lock` an, führt `cargo update` aus und erstellt direkt einen Commit. So lassen sich Codex-Konflikte mit automatisch regenerierten Lockfiles vermeiden.
- Für manuelle Flows: `git add . ':!Cargo.lock' && git commit …`, danach `cargo update`, `git add Cargo.lock` und einen separaten „refresh“-Commit anlegen.

### Sprache
- Primärsprache ist Deutsch (Du-Form, klare Sätze), Code-Kommentare und Log-Meldungen bleiben Englisch.
- Gender-Sonderzeichen (`*`, `:`, `·`, `_`, Binnen-I) sind tabu; nutze neutrale Formulierungen.
- Details und Prüfschritte findest du in [`docs/process/sprache.md`](docs/process/sprache.md); Vale läuft im CI sowie lokal über `vale .`.

---

## Weiterführende Dokumente
- [`hauski-skizze.md`](hauski-skizze.md) – Vision, Funktionsumfang, Performance-Budgets, Security-Ansatz.
- [`hauski-stack.md`](hauski-stack.md) – Technologiewahl, Tooling, CI-Strategie und Testpyramide.

