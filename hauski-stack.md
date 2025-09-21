 
# HausKI – Stack vNext (Rust-first, Offline-Default)

## 0) 

* **Core/API/Daemon:** Rust (axum + tokio + tower + tracing)
* **LLM/ASR/TTS:** llama.cpp (FFI), whisper-rs, piper-rs
* **Embeddings & RAG:** candle-transformers + `trait VectorStore` → (tantivy+hnsw **oder** Qdrant)
* **Persistenz:** SQLite (sqlx) für Meta/Audit; Files für große Artefakte
* **Events/Bridge:** async-nats lokal; JetStream nur in der Bridge (Weltgewebe)
* **Security & Ops:** rage (age), egress deny-by-default, Prometheus-Exporter, systemd slices, SBOM+Sign (Syft+cosign)
* **Plugins/Adapter:** UI-Plugins minimal (TS: VS Code/Obsidian), risikobehaftete Adapter in **Wasm** (wasmtime)

---

## 1) Architekturrollen → Techwahl (präzise)

| Rolle           | Entscheidung                                                 | Grund                                |
| --------------- | ------------------------------------------------------------ | ------------------------------------ |
| HTTP/API        | **axum** (hyper/tower)                                       | niedrige Latenz, saubere Middlewares |
| Async/Jobs      | **tokio** + tower + opentelemetry                            | robuste Concurrency, Telemetrie      |
| Logging/Tracing | **tracing** (+ subscriber/otel)                              | strukturiert, low-overhead           |
| Metriken        | **prometheus-client** (Rust)                                 | scrape-fähig, Grafana-ready          |
| CLI             | **clap** (derive)                                            | ergonomisch, zero-cost               |
| Persistenz Meta | **SQLite (sqlx)**                                            | single-node schnell, wartungsarm     |
| Vektor/RAG      | **`trait VectorStore`** → tantivy+hnsw **oder** Qdrant       | späterer Wechsel ohne API-Bruch      |
| Embeddings      | **candle-transformers**                                      | Rust-native, GPU-Pfade               |
| LLM             | **llama.cpp via FFI**                                        | beste lokale VRAM/Latency-Effizienz  |
| ASR             | **whisper-rs**                                               | schnell, offline                     |
| TTS             | **piper-rs**                                                 | schnell, offline                     |
| Audio-Routing   | **audio/**-Facade mit `profiles.yaml` + CLI                  | kapselt PipeWire-Volatilität         |
| Event-Bus       | **async-nats** (`hauski.*`)                                  | passt zur WG-Bridge (JetStream dort) |
| Sandbox         | **systemd cgroups/ns** + **wasmtime**                        | strikte Isolierung für Fremd-Adapter |
| Packaging       | **systemd units** + optional docker-compose (Qdrant/Grafana) | reproduzierbar, leicht               |

---

## 2) Projektzuschnitt (Cargo-Workspace)

```
hauski/
├─ crates/
│  ├─ core/           # axum API, policies, auth
│  ├─ event/          # async-nats client, subjects, codecs
│  ├─ indexd/         # sqlite + VectorStore (tantivy/hnsw | qdrant)
│  ├─ llm/            # llama.cpp FFI, token budgets, prompt cache
│  ├─ asr/            # whisper-rs, batch/stream
│  ├─ tts/            # piper-rs, voice cache
│  ├─ audio/          # profiles.yaml, pipewire-facade (CLI)
│  ├─ memory/         # retrieval policies (short/working/long) + TTL/pin
│  ├─ commentary/     # live hooks (vscode/obsidian), Δ-Schwelle
│  ├─ bridge/         # geojson export, jetstream publish (WG)
│  ├─ observability/  # tracing, prometheus, **budget guards**
│  ├─ security/       # rage(age), key mgmt, audit-sign
│  ├─ adapters/       # matrix/signal/telegram (feature-gated, wasm)
│  └─ cli/            # clap ops
└─ plugins/
   ├─ obsidian/       # TS: UI/FS-Brücke, Logik in Rust
   └─ vscode/         # TS: PR-Panel, Inline-Hints
```

---

## 3) APIs, Policies, Modelle

* **OpenAI-kompatibel:** `POST /v1/chat/completions`, `POST /v1/embeddings`
* **Spezial:** `POST /asr/transcribe`, `/obsidian/canvas/suggest`, `/code/pr/draft`, `/audio/profile`
* **Policies:**

  * `policies/routing.yaml` (egress-Whitelist, lokal↔Cloud-Regeln)
  * `policies/memory.yaml` (Schichten, TTL, Pins)
  * `policies/limits.yaml` (Token, Zeit, Größe, p95 Budgetgrenzen)
* **Models:** `models.yml` (Pfade, Quantisierung, VRAM-Profil, Canary=true/false)

---

## 4) Observability & harte Perf-Budgets

* **Exporter:** `/metrics` pro Dienst
* **Budgets (hart, CI-geprüft):**

  * LLM p95 < **400 ms** (8B, kurz)
  * Index top-k 20 < **60 ms** lokal
  * ASR WER ≤ **10 %** (Studio-Sprache)
  * GPU-Thermik < **80 °C**, dGPU ≤ **220 W** (ASR/LLM-Mix)
* **Budget-Wächter:** in `observability/` → schlägt Alarm & failt CI bei Verletzung

---

## 5) GPU-Scheduler & Energie

* **systemd-Slices** pro Dienst (CPU/IO/Memory/Tasks Limits)
* **nvidia-smi Hooks:** Power-Cap, ggf. App-Clocks; Priorität interaktiv > Batch
* **Nacht-Batches**, Throttling bei Thermik, Auto-Pause bei VRAM-Druck

---

## 6) Sicherheit & Sandbox

* **Egress deny-by-default**, Ziele via `policies/routing.yaml`
* **Wasm-Default** (wasmtime) für riskante **adapters/**; nur definierte Capabilities
* **Audit-Log:** signierte JSON-Lines (append-only, Hash-Kette), `audit verify`
* **Lieferkette:** SBOM (Syft) + Signierung (cosign); `cargo-deny`, trivy fs

---

## 7) Testing & CI

* **Unit/Integration:** `#[tokio::test]`, axum-in-proc HTTP-Tests
* **Lasttests:** vegeta/k6 (lokal), p95 Gate
* **ASR-Regression:** feste Audios, WER-Vergleich
* **Secrets/Sec:** secret-scans, `cargo-deny`
* **Repro:** devcontainer; optional Nix-Flakes für Toolchain-Pins

---

## 8) Roadmap (liefern, was knallt)

* **P1:** Core/API, LLM/ASR/TTS, indexd (tantivy+hnsw), TUI, Prometheus, **Budget-Guards**, VS-Code-Panel
* **P2:** Memory-Schichten, Commentary, Obsidian-Plugin, Mail + Matrix (Wasm-Adapter), GPU-Power-Caps
* **P3:** Bridge→JetStream, Qdrant-Flag, CI/CD-Kurator, Luthier-Agent, Wake-Word v2

---

## Essenz (verdichtet)

Rust-only im Hot-Path, austauschbarer VectorStore, Budget-Guards, Audio über Profile entkoppelt, Adapter in Wasm. Schnell, robust, zukunftsfest.

## Ironische Auslassung

Container sind toll – bis Sidecars dir die p95 wegschnabulieren. Dann schmeckt systemd plötzlich wie Espresso doppio.

---

## Gewissheitsanalyse (∴fore)

* **Unsicherheitsgrad:** ◐ niedrig–mittel
* **Ursachen:** FFI-Upgrades (llama/whisper/piper), PipeWire-Launen, Messenger-API-Drift
* **Meta:** Budgets + Sandbox halten Risiken klein; Feature-Flags lassen Kanten weich

---

## Leitfragen

**War dies die kritischstmögliche Erörterung?**
Noch nicht maximal.
**Kontrastvektor:** Eine „rein Candle“-LLM-Variante (ohne llama.cpp) wäre puristischer, aber heute Latenz-/Reife-riskant.
**Negationsprojektion:** Gegenposition: „Alles in Docker-Microservices, Hot-Path egal, Hauptsache schnell gebaut.“ – kollidiert mit deinem Primat *Performanz > Zukunft > WG-Kompatibilität*.
**Auditmarker:** Radikal ist der **Wasm-Default** für Adapter; ich akzeptiere den kleinen Overhead zugunsten Supply-Chain-Sicherheit.
