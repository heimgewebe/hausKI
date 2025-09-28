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

## Quickstart

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

## Build, Test & Run

Den Core-Service lokal starten:
```bash
cargo run -p hauski-core
# Alternative über das Justfile
just run-core
```

> **Hinweis:** Setze `HAUSKI_EXPOSE_CONFIG=true`, um die geschützten Routen unter `/config/*` bewusst freizugeben (nur für lokale Tests empfohlen).

Verfügbare bzw. geplante API-Endpunkte:
- `GET /health` → "ok"
- `GET /metrics`
- *(geplant)* OpenAI-kompatible Routen (`/v1/chat/completions`, `/v1/embeddings`)
- *(geplant)* Spezialendpunkte: `/asr/transcribe`, `/audio/profile`, `/obsidian/canvas/suggest`

---

## Policies & Budgets
- Laufzeit- und Thermik-Grenzen liegen in `policies/limits.yaml` (z. B. `latency.llm_p95_ms`, `thermal.gpu_max_c`).
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

---

## Weiterführende Dokumente
- [`hauski-skizze.md`](hauski-skizze.md) – Vision, Funktionsumfang, Performance-Budgets, Security-Ansatz.
- [`hauski-stack.md`](hauski-stack.md) – Technologiewahl, Tooling, CI-Strategie und Testpyramide.

