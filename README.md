![WGX](https://img.shields.io/badge/wgx-enabled-blue)
<!-- Coverage and Security badges will be added once the corresponding workflows are available. -->

# HausKI — Rust-first, Offline-Default, GPU-aware

HausKI ist ein lokaler KI-Orchestrator für Pop!_OS-Workstations mit NVIDIA-RTX-GPU.

> **📋 Implementierungsstatus:** Nicht alle Features sind bereits implementiert.
> Siehe [`docs/ist-stand-vs-roadmap.md`](docs/ist-stand-vs-roadmap.md) für eine vollständige
> Übersicht über den aktuellen Stand (✅) und geplante Erweiterungen (🔮).

**Hauptmerkmale (aktuell implementiert):**
- **Rust-basiert**: Der Kern des Orchestrators ist in Rust implementiert und nutzt `axum` und `tokio` für hohe Performance und Sicherheit.
- **Offline-First**: Entwickelt für den lokalen Betrieb ohne ständige Internetverbindung.
- **In-Memory-Indexierung**: Substring-basierte Suche mit Namespace-Support (Vektorsuche geplant für P2).
- **Decision-Weighting**: Intelligente Gewichtung von Suchtreffern nach Trust, Recency und Context (siehe [`docs/decision-weighting.md`](docs/decision-weighting.md)).
- **Memory-System**: SQLite-basierter Key-Value-Store mit TTL und Pin/Unpin-Mechanismus.
- **Policy-Engine**: Ein regelbasiertes System zur Steuerung von Routing, Speicher und Systemgrenzen.
- **Egress-Guard**: Whitelist-basierte Kontrolle ausgehender HTTP-Requests.
- **Observability**: Prometheus-Metriken für HTTP, Memory und Index-Latenz.

**Geplante Features (Roadmap):**
- **GPU-Unterstützung**: Optimiert für NVIDIA-GPUs zur Beschleunigung von KI-Inferenz-Aufgaben (P1-P2).
- **Vektorsuche**: Integration von `tantivy+hnsw` oder Qdrant für semantische Suche (P2).
- **LLM/ASR/TTS**: Lokale Inference mit llama.cpp, whisper-rs, piper-rs (P1).
- **Plugins & Cloud-Fallback**: Erweiterbare Plugin-Architektur und Cloud-Routing (P2).

### Lokaler Chat (llama.cpp Upstream)

**Voraussetzungen:**
- `tmux` (für parallele Upstream-/Core-Sitzung)
- `lsof` (für das Stop-Skript)
- Optional: `just` (verkürzt Startaufrufe)

Für einen lokalen, OpenAI-kompatiblen Chat-Pfad nutze das Runbook unter
`docs/runbooks/local-chat.md` **oder** starte alles per:

```bash
scripts/start-all.sh --model "$HOME/models/your-model.gguf" --port 8081
```

**Extras**:
- `--tmux` startet eine Session `hauski` (Fenster: `upstream`, `core`) – ideal zum Debuggen.
- `.env`/`configs/.env` erlauben Start ohne Flags (siehe `.env.example`).
- `--upstream-url` oder `UPSTREAM_URL=…` verbindet gegen einen bestehenden Dienst; externe URLs deaktivieren den lokalen Start automatisch (optional mit `--no-upstream` erzwingbar).
- Schreibt eine Flags-Datei (unter `~/.config/hauski/hauski-flags.yaml`) und setzt `HAUSKI_FLAGS` automatisch auf `chat_upstream_url` (bestehende `HAUSKI_FLAGS`-Werte werden weiterverwendet).
- Logs inkl. einfacher Rotation unter `~/.local/state/hauski/logs/`.
- Stoppen mit:

```bash
scripts/stop-all.sh        # nutzt tmux, sonst Ports/PIDs
```

---

## Inhalt
- [Quickstart](#quickstart)
- [Entwicklung im Devcontainer](#entwicklung-im-devcontainer)
- [Build, Test & Run](#build-test--run)
- [Memory & semantische Suche](#memory--semantische-suche)
- [Policies & Budgets](#policies--budgets)
- [Modelle & Speicherorte](#modelle--speicherorte)
- [Architektur & Verzeichnisse](#architektur--verzeichnisse)
- [Projektstruktur](#projektstruktur)
- [Roadmap-Fokus](#roadmap-fokus)
- [Contribution & Qualität](#contribution--qualität)
- [Weiterführende Dokumente](#weiterführende-dokumente)

---

## Server-Tunables (per Umgebungsvariable)

| Variable                    | Typ | Default | Wirkung |
|----------------------------|-----|---------|--------|
| `HAUSKI_HTTP_TIMEOUT_MS`   | u64 | `1500`  | Request-Timeout in Millisekunden (bei `0` deaktiviert) |
| `HAUSKI_HTTP_CONCURRENCY`  | u64 | `512`   | Limit gleichzeitiger Requests (bei `0` deaktiviert) |

Beispiel:

```bash
HAUSKI_HTTP_TIMEOUT_MS=2500 HAUSKI_HTTP_CONCURRENCY=256 ./target/release/hauski-cli serve
```

## Schnellstart

**Voraussetzungen lokal (Pop!_OS, Rust stable):**
```bash
rustc --version && cargo --version
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo build --workspace
cargo test --workspace -- --nocapture
```

### Python Shadow Policy API

1. Optional die Python-Extras synchronisieren (uv verwaltet automatisch eine lokale Umgebung):

   ```bash
   uv sync --extra dev --locked --frozen
   ```

   > 💡 Tipp: `--locked` stellt sicher, dass exakt die im `uv.lock` festgeschriebenen Versionen
   > installiert werden – identisch zu unseren CI-Läufen. `--frozen` bricht zusätzlich ab,
   > falls `pyproject.toml` und `uv.lock` auseinanderlaufen.

2. Den FastAPI-Dienst lokal starten:

   ```bash
   uv run uvicorn services.policy_shadow.app:app --reload --port 8085
   ```

   Alternativ kannst du `just shadow` verwenden. Setze bei Bedarf `HAUSKI_TOKEN=supersecret` und rufe den Dienst anschließend mit dem Header `x-auth: <token>` auf.

3. Beispielaufrufe mit `curl`:

   ```bash
   curl -X POST http://127.0.0.1:8085/v1/ingest/metrics \
     -H 'Content-Type: application/json' \
     -d '{
       "ts": 1700000000000,
       "host": "pop-os",
       "updates": {"packages": 3},
       "backup": {"last_run": "2024-01-01"},
       "drift": {"config": []}
     }'

   curl -X POST http://127.0.0.1:8085/v1/policy/decide \
     -H 'Content-Type: application/json' \
     -d '{"context": {"routine": "coffee"}}'

   curl -X POST http://127.0.0.1:8085/v1/policy/feedback \
     -H 'Content-Type: application/json' \
     -d '{"decision_id": "abc", "rating": 1.0, "notes": "works"}'

   curl http://127.0.0.1:8085/v1/health/latest
   curl http://127.0.0.1:8085/version
   ```

> 💡 **Hinweis auf Offline-Builds:** Bevor du `cargo clippy`, `cargo build` oder
> `cargo test` ausführst, stelle sicher, dass `vendor/` alle benötigten Crates
> enthält. Der Helper `scripts/check-vendor.sh` warnt früh mit einer
> verständlichen Meldung, falls beispielsweise `axum` noch nicht lokal
> vorliegt. Standardmäßig lädt Cargo fehlende Crates wieder aus `crates.io`;
> setze `HAUSKI_ENFORCE_VENDOR=1`, wenn der Build zwingend offline erfolgen
> soll.

> Falls CI mit der Meldung `the lock file … needs to be updated but --locked was
> passed` oder `no matching package named 'axum' found` stoppt, führe die
> Aktualisierung lokal durch und committe die Ergebnisse:
> 1. `cargo generate-lockfile` (bzw. `cargo update`), um die `Cargo.lock` zu
>    erneuern.
> 2. `cargo vendor` (oder `just vendor`), damit `vendor/` alle Crates enthält.
> 3. `git add Cargo.lock vendor/` und anschließend committen.

```toml
# .cargo/config.toml
[registries.crates-io]
protocol = "sparse"

[source.vendored-sources]
directory = "vendor"
```

> Offline-Builds kannst du erzwingen, indem du Cargo mit `--config` oder einer
> eigenen `config.toml` startest, die `source.crates-io.replace-with =
> "vendored-sources"` setzt. So bleiben air-gapped Workflows weiterhin möglich.

**Vendor-Snapshot befüllen**

Mit Internetzugang lässt sich der Snapshot direkt im Repository erzeugen:

```bash
just vendor
just vendor-archive
```

Die erzeugte Datei `hauski-vendor-snapshot.tar.zst` kannst du anschließend auf
eine Offline-Maschine kopieren und dort auspacken:

```bash
mkdir -p vendor
tar --zstd -xvf hauski-vendor-snapshot.tar.zst -C vendor --strip-components=1
```

Alternativ steht der Snapshot auch als Artefakt des Workflows
`vendor-snapshot` zur Verfügung.

**VS Code Devcontainer:**
1. Repository klonen und in VS Code öffnen.
2. "Reopen in Container" ausführen; das Post-Create-Skript setzt `pre-commit` auf und prüft GPU-Verfügbarkeit.
3. Danach genügen die Shortcuts aus der `justfile` (`just build`, `just test`, `just run-core`).

### Codex-Review-Ablage

Codex-Läufe schreiben ihre Rohdaten nach `~/.hauski/review/hauski/`. Lege dir im Repo optional einen Symlink von `~/.hauski/review/hauski` auf `.hauski-reports` an; dadurch bleiben Logs, Pläne und Canvas-Dateien persistent, ohne ins Repo zu geraten.

```bash
ln -s ~/.hauski/review/hauski .hauski-reports
```

Nutze `just codex:doctor`, um vor einem Run schnell zu prüfen, ob eine lokale `codex`-Installation gefunden wird oder automatisch auf `npx @openai/codex@0.77.0` zurückgefallen wird.

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
- Basis-Tooling: Rust, Node, Python, uv sowie optionale CUDA-Runtime.
- Empfohlene Extensions: rust-analyzer, direnv, Docker, GitLens, Markdown-Mermaid.
- Vorgefertigte Tasks: `cargo: fmt`, `cargo: clippy`, `cargo: build`, `cargo: test`.
- Just-Shortcuts: `just fmt`, `just lint`, `just build`, `just test`, `just run-core`.
- Netzwerkzugriffe von Cargo nutzen standardmäßig den Sparse-Index (`CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`), um strenge Proxies zu umgehen; `CARGO_NET_RETRY=5` sorgt für automatische Wiederholungen.
- `cargo-deny` ist im Container global installiert und steht ohne zusätzliche Schritte bereit.

---

## Build, Test & Ausführung

Den Core-Service lokal starten (CLI-Kommando `hauski serve`):
```bash
cargo run -p hauski-cli -- serve
# Alternative über das Justfile (ruft intern `cargo run -p hauski-cli -- serve` auf)
just run-core
```

Nach dem Start stehen folgende Kern-Endpunkte zur Verfügung:

| Route | Methode | Zweck |
| --- | --- | --- |
| `/health` | GET | Lebenszeichen, antwortet mit `ok`. |
| `/healthz` | GET | Lightweight-Check für Load-Balancer/Probes. |
| `/ready` | GET | Signalisiert, dass Limits, Modelle & Routing geladen wurden. |
| `/metrics` | GET | Prometheus-kompatible Metriken (HTTP, Budgets). |
| `/v1/chat` | POST | Erster Chat-Stub. Gibt aktuell `501 Not Implemented` zurück und demonstriert den JSON-Schema-Fluss. |
| `/docs`, `/api-docs/openapi.json` | GET | Menschliche bzw. maschinenlesbare API-Dokumentation (alias: `/docs/openapi.json` → 308 Redirect). |

Beispiel-Aufrufe:

```bash
curl -i http://127.0.0.1:8080/health
curl -i http://127.0.0.1:8080/healthz
curl -s http://127.0.0.1:8080/metrics | head -n 20

curl -s -X POST http://127.0.0.1:8080/v1/chat \
  -H 'Content-Type: application/json' \
  -d '{"messages":[{"role":"user","content":"Hallo HausKI?"}]}'
# → HTTP 501 + JSON-Payload {"status":"not_implemented", …}
```

> **Hinweis:** Setze `HAUSKI_EXPOSE_CONFIG=true`, um die geschützten Routen unter `/config/*` bewusst freizugeben (nur für lokale Tests empfohlen).

### Chat-Upstream aktivieren (echte Antworten)

Du kannst `/v1/chat` an einen **OpenAI-kompatiblen lokalen Server** (z. B. `llama.cpp --server`) anbinden:

1. **Llama-Server starten** (Default-Modellpfad bei Bedarf anpassen):

   ```bash
   just llama-server MODEL=/opt/models/llama3.1-8b-q4.gguf PORT=8081
   ```

2. **Flag setzen** in `configs/flags.yaml`:

   ```yaml
   safe_mode: false
   chat_upstream_url: "http://127.0.0.1:8081"
   ```

   Alternativ via Env: `export HAUSKI_FLAGS=./configs/flags.yaml`

3. **Core starten**:

   ```bash
   just run-core
   ```

4. **Chat testen** (liefert jetzt `200` mit Inhalt):

   ```bash
   just chat-demo TEXT="Erzähl mir einen kurzen Hauswitz."
   ```

Ohne laufenden Upstream bleibt `/v1/chat` weiterhin bei `501 Not Implemented` (sicherer Fallback).

### Optional: systemd-User-Service für den Core

Für einen dauerhaften lokalen Betrieb kannst du den Core über systemd (user scope) starten:

```bash
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/hauski-core.service <<'UNIT'
[Unit]
Description=HausKI Core (Rust)
After=network.target

[Service]
Environment=HAUSKI_ALLOWED_ORIGIN=http://127.0.0.1:8080
Environment=HAUSKI_MODELS=%h/projects/hauski/configs/models.yml
Environment=HAUSKI_LIMITS=%h/projects/hauski/policies/limits.yaml
Environment=HAUSKI_ROUTING=%h/projects/hauski/policies/routing.yaml
Environment=HAUSKI_FLAGS=%h/projects/hauski/configs/flags.yaml
ExecStart=%h/.cargo/bin/hauski serve
WorkingDirectory=%h/projects/hauski
Restart=on-failure

[Install]
WantedBy=default.target
UNIT

systemctl --user daemon-reload
systemctl --user enable --now hauski-core.service
systemctl --user status hauski-core.service --no-pager
```

> Passe Pfade und Ports bei Bedarf an deine lokale Umgebung an. `hauski serve` bleibt auf Loopback gebunden, solange keine andere Adresse per `HAUSKI_BIND` gesetzt wird.

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
- `GET /healthz`
- `GET /ready`
- `GET /metrics`
- `POST /v1/chat` *(501 Stub – LLM-Binding folgt)*
- *(geplant)* OpenAI-kompatible Routen (`/v1/chat/completions`, `/v1/embeddings`)
- *(geplant)* Spezialendpunkte: `/asr/transcribe`, `/audio/profile`, `/obsidian/canvas/suggest`

### Observability & Metriken

Die API exportiert Prometheus-kompatible Kennzahlen unter `/metrics`:

- `http_requests_total{method,path,status}` zählt eingehende HTTP-Requests pro Methode, Route und Statuscode.
- `http_request_duration_seconds{method,path}` erfasst Latenzen als Histogramm mit den Standard-Buckets `0.005s` bis `1s`.

Beispielabfragen für Dashboards oder die Prometheus-Konsole:

- Erfolgs- vs. Fehlerraten (Beispiel in PromQL):
  ```promql
  sum by (status) (rate(http_requests_total[5m]))
  ```
- 95%-Perzentil der Request-Latenz je Route (Beispiel in PromQL):
  ```promql
  histogram_quantile(0.95, sum by (le, method, path) (rate(http_request_duration_seconds_bucket[5m])))
  ```

## Lint & Tests
- Formatierung: `cargo fmt --all`.
- Lints: `cargo clippy --all-targets --all-features -- -D warnings` und `cargo deny check`.
- Tests: `cargo test --workspace -- --nocapture`.
- **Python-Tooling (optional):**
  - Setup:
    ```bash
    uv sync --extra dev --locked --frozen
    uv run pre-commit install
    ```
  - Init: `just py-init`
  - Lint: `just py-lint`
  - Format: `just py-fmt`
  - Tests: `just py-test`
  - Docs lokal: `just py-docs-serve`
  - Docs strikt: `just py-docs-build`
  - Hooks lokal prüfen: `just py-pre-commit`


## Review-Daemon

Ein systemd-User-Timer startet alle fünf Minuten genau einen Review-Lauf und verhindert Parallelität per Lockfile `~/.hauski/review/hauski.lock`. Logs landen unter `~/.hauski/review/hauski/review-YYYYmmdd-HHMMSS.log`.

```bash
just reviewd:install
just reviewd:enable
just reviewd:status
```

Der Timer lädt optional Variablen aus `.env` im Repo-Root und bevorzugt `just review-quick` (Fallback auf `just review`). Perspektivisch ist ein Wechsel auf eine Path-Unit oder ein eventgetriebenes Setup (Git-Hook, inotify) vorgesehen.

---

## Review-Zyklus & Flags
- Speicherpfade: Reports landen unter `~/.hauski/review/<repo>/`, optionaler Symlink `.hauski-reports` hält sie außerhalb des Repos.
- Index: `~/.hauski/review/index.json` führt alle Läufe zusammen.
- Hook & flock: Git-Hooks nutzen `flock` und erkennen Doc-only-Changes.
- Async-Mode: `HAUSKI_ASYNC=1` legt Läufe in die Queue und gibt schneller frei.
- Ausblick: `hauski-reviewd` soll als Daemon das Review-Verzeichnis beobachten.

Siehe [docs/review-cycle.md](docs/review-cycle.md) für Details.
Der optionale Upstream für Chat lässt sich via `chat_upstream_url` setzen.

---

## Memory & semantische Suche

HausKI bringt mit [semantAH](docs/semantah.md) eine semantische Gedächtnisschicht mit. Der Bootstrap enthält Dokumentation, Konfiguration, Skripte und Rust-Scaffolds für Index, Graph und Related-Blöcke. Starte mit dem Quickstart in `docs/semantah.md`, um Ollama einzubinden, Seeds zu laden und die `/index`-Endpunkte zu testen.

### Fragen stellen (Semantik-Suche)

```bash
curl -s "http://localhost:8080/ask?q=dein+text&k=5&ns=default" | jq
```

Der Endpoint liefert die Top-k-Treffer mit Score, Snippet und Metadaten aus dem lokalen Index. Der Server begrenzt `k` serverseitig auf den Bereich 1–100 Treffer und spiegelt das effektive Limit im Response-Feld `k` wider.

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

## Projektstruktur

Das `hauski`-Repository ist als Cargo-Workspace organisiert, um eine klare Trennung der Verantwortlichkeiten zu gewährleisten. Jedes Crate erfüllt einen bestimmten Zweck und trägt zur Gesamtarchitektur bei:

- **`crates/cli`**: Die Kommandozeilenschnittstelle (`CLI`), die mit `clap` erstellt wurde. Sie dient als Einstiegspunkt für Benutzerinteraktionen.
- **`crates/core`**: Der Kern-Service, der mit `axum` entwickelt wurde. Er enthält die zentrale Geschäftslogik, API-Endpunkte und die Policy-Engine.
- **`crates/embeddings`**: Verantwortlich für die Erstellung von Vektor-Embeddings aus Textdaten.
- **`crates/indexd`**: Ein Dienst, der `SQLite` und `tantivy` nutzt, um eine effiziente Indizierung und Suche zu ermöglichen.
- **`crates/policy`**: Definiert die Datenstrukturen und die Logik für das Policy-System.
- **`crates/policy_api`**: Stellt eine API zur Interaktion mit der Policy-Engine bereit.

---

## Deployment

Das Deployment von `hauski` kann auf verschiedene Weisen erfolgen, je nach den Anforderungen der Zielumgebung.

### Lokales Deployment

Für die lokale Entwicklung und Nutzung wird empfohlen, `hauski` direkt aus den Quellen zu erstellen und auszuführen. Die `justfile` bietet eine Reihe von Befehlen zur Vereinfachung dieses Prozesses.

### Docker-Compose

Für eine produktionsnahe Umgebung kann `hauski` mit `docker-compose` bereitgestellt werden. Das Repository enthält eine `docker-compose.yml`-Datei, die die erforderlichen Dienste definiert.

### Systemd

Für eine dauerhafte Installation kann `hauski` als `systemd`-Dienst konfiguriert werden. Dies stellt sicher, dass der Dienst beim Systemstart automatisch gestartet wird und bei Fehlern neu gestartet wird.

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
- [`docs/ist-stand-vs-roadmap.md`](docs/ist-stand-vs-roadmap.md) – **Implementierungsstatus**: Übersicht über alle Features mit klarer Trennung zwischen Ist-Stand (✅) und Roadmap (🔮).
- [`docs/hauski-skizze.md`](docs/hauski-skizze.md) – Vision, Funktionsumfang, Performance-Budgets, Security-Ansatz (Roadmap-fokussiert).
- [`docs/hauski-stack.md`](docs/hauski-stack.md) – Technologiewahl, Tooling, CI-Strategie und Testpyramide (inkl. Status-Marker).
- [`docs/inconsistencies.md`](docs/inconsistencies.md) – Dokumentierte Abweichungen zwischen Architektur-Dokumenten und aktueller Implementierung.
- [`docs/vision/multi-agent-rag.md`](docs/vision/multi-agent-rag.md) – Orchestrierung der spezialisierten Agenten inkl. Contracts.
  Einstieg über `just agents.sync` (Template spiegeln) und `just agents.run` (Dry-Run).

### Python-Dev Abhängigkeiten & Lockfile (uv)
- CI erwartet **reproduzierbare** Installs: `uv sync --extra dev --locked --frozen`.
- Wenn du `pyproject.toml` änderst (z. B. neue Dev-Lib wie `jsonschema==4.23.0`):
  1. `uv lock --extra dev`
     _oder gezielt:_ `uv lock --upgrade-package jsonschema==4.23.0`
  2. `git add uv.lock`
  3. commit & push

Ohne aktualisiertes `uv.lock` bricht CI mit einem klaren Hinweis ab.

## Systemkontext

Der aktuelle Zweck, Lifecycle-Status und die Beziehungen dieses Repositories zu anderen
Heimgewebe-Systemen werden im [Systemkatalog](https://github.com/heimgewebe/systemkatalog) geführt. Die
[gerenderte Systemübersicht](https://github.com/heimgewebe/systemkatalog/blob/main/rendered/system-catalog.md)
ist die lesbare Gesamtsicht; die
[maschinenlesbare Inventur](https://github.com/heimgewebe/systemkatalog/blob/main/registry/ecosystem/nodes.json)
ist die Quelle für Automatisierung.

Repositoryeigene Betriebs-, Daten- und Implementierungswahrheit bleibt in diesem Repository.
Gemeinsame Contracts bleiben bei ihrer jeweiligen Primärquelle.
