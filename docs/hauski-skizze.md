# HausKI – Skizze vNext (Rust-first, Pop!\_OS, RTX 4070 Ti)

> **⚠️ Hinweis:** Dieses Dokument beschreibt die **Architektur-Vision** von HausKI.
> Nicht alle hier beschriebenen Features sind bereits implementiert.
> Für den **aktuellen Implementierungsstatus** siehe [`ist-stand-vs-roadmap.md`](ist-stand-vs-roadmap.md).

## 0) Kurzfassung

HausKI ist ein **lokaler KI-Orchestrator** mit strengem Offline-Default. Hot-Path vollständig in **Rust**.
Inference: **llama.cpp** (GGUF, CUDA), **whisper-rs** (ASR), **piper-rs** (TTS). **(🔮 Geplant P1)**
Wissen: **SQLite** + **tantivy+hnsw** (leicht) → optional **Qdrant** via Feature. **(🔮 Geplant P2)**
UX: **TUI (ratatui)**, **VS-Code-Extension**, **schlankes Obsidian-Plugin**. **(🔮 Geplant P3)**
Policies regeln lokal↔Cloud. **GPU-Scheduler** + **CPU-Fallback** sichern Realtime. **(🔮 Geplant P2)**
Neu: `trait VectorStore`, `audio/profiles.yaml`, **Wasm-Default** für riskante Adapter, **harte p95/p99-Budgets**. **(🔮 Geplant P1-P2)**

---

## 1) Funktionen (Was HausKI tut)

### 1.1 Orchestrator & System

* **Policy-Router** lokal↔Cloud (Default: lokal; Kriterien: Privatsphäre, Kosten, Latenz, Qualitätsziel).
* **GPU-Scheduler**: VRAM-Quoten, Power-Caps, Prioritäten (interaktiv > Batch), Thermik-Wächter.
* **Fallbacks**: bei VRAM/Hitze → CPU-Inference (candle/int8).
* **Health-Daemon**: journald-Triage, Paket-Drift, Self-Heal-Hints, Snapshots/Restore.

### 1.2 Gedächtnis & Live-Kommentar

* **Memory-Schichten**: `short_term`, `working_context`, `long_term` (TTL, Pins, Themen-Buckets).
* **On-the-fly-Kommentierung** (VS Code/Obsidian): Δ-Schwelle, Cooldowns, Spam-Bremse.
* **Konflikt-Detektor**: erkennt widersprüchliche Notizen, verlinkt Quellen.
* **Modus „Prof. Dr. Kranich“**: Gegenposition + sichtbarer Unsicherheitsgrad (∴fore).

### 1.3 Obsidian / Canvas

* **Text→Knoten**, Auto-Layout, Domänen-Farben.
* **Mindmapception**: Sub-Canvas mit Backlinks.
* **Graph-Lint & Canvas-Diff**: Duplikate, Waisen, Unbalance, tote Verweise.
* **Semantische Suche** (Hybrid Vektor+Symbolik) & **Narrativ-Generator** (Briefing/PRD/Pitch).

### 1.4 Code & DevOps

* **PR-Drafter**: Titel, Changelog, Testplan, Reviewer-Checkliste.
* **Review-Copilot (lokal)**: Semgrep-Heuristiken, Leak-Check, Policy-Hints.
* **CI/CD-Kurator**: Caches, Matrix, Concurrency, Smoke-Tests-Vorschläge.
* **Repo-Navigator**: „Erkläre Init-Sequenz“, „Finde Event-Store-Zugriffe“.

### 1.5 Audio (MOTU M2)

* **Profile**: Hi-Fi (Qobuz), Unterricht (Loopback+Mic), Aufnahme (Low-Latency).
* **ASR lokal**: Whisper (GPU), SRT/Markdown mit Timestamps.
* **Luthier-Agent**: Akkord/Tempo-Skizzen, Übungs-Marker.
* **TTS**: Piper lokal.
* **Wake-Word** (optional, prozess-separiert, ohne Cloud).

### 1.6 Kommunikation (Adapter optional)

* **Mail**: IMAP-Pull (mbsync → Maildir), Zusammenfassungen, Antwort-Entwürfe; ICS-Extraktion.
* **Messenger** (Feature-Flags): Matrix, Signal (signal-cli), Telegram.
* **Smart-Benachrichtigungen**: Entscheidungen/Fristen erkennen; Threads ↔ Obsidian-Knoten verknüpfen.
* **RSS lokal**: Repo-Commits, News, Kalenderfeeds.

### 1.7 Weltgewebe-Bridge

* **GeoJSON-Exporter** (Knoten/Fäden; Sichtbarkeitsstufen).
* **NATS-Publisher** (Subjects `hauski.*` → JetStream).
* **„Cui Bono“-Prüfer**: markiert Machtannahmen/Interessen.

### 1.8 Sicherheit

* **No-Leak-Guard**: Egress **deny-by-default**, Whitelist je Ziel.
* **KMS-mini**: `rage` (age) + **Audit-Signaturen** (append-only JSON-Lines).
* **RBAC**: Admin/Operator/Gast.
* **Sandboxing**: systemd cgroups/Namespaces; **Default: Wasm (wasmtime)** für Fremd-Adapter.

---

## 2) Technik (Wie das zusammenhängt)

### 2.1 Architektur (ASCII)

```
Clients (TUI • VSCode • Obsidian • CLI • Audio • Comm)
        │
        ▼
┌──────────────────────────┐
│ core/ (axum)             │  API, Policies, Auth
│ event/ (async-nats)      │  Subjects, Codecs
│ security/                │  rage(age), audit
└─────┬─────────┬──────────┘
      │         │
   indexd/    daemon/        bridge/
 (SQLite+     (backup,       (GeoJSON,
  vector)      audio,         NATS)
               comm,
               comment)
      │
      ▼
 llm/ (llama.cpp FFI) • asr/ (whisper-rs) • tts/ (piper-rs)
```

### 2.2 Module (Crates)

**✅ Implementiert:**
* `core/` (axum API, Policy-Engine, Auth, HTTP-Endpoints)
* `indexd/` (In-Memory-Index mit Substring-Suche, Namespace-Support)
* `memory/` (SQLite Key-Value-Store, TTL, Pin/Unpin-Mechanismus)
* `policy/` (Policy-Datenstrukturen)
* `policy_api/` (Policy-API-Layer, optional `heimlern` Feature)
* `cli/` (clap-basierte CLI-Tools)
* `embeddings/` (Basis-Struktur)

**🔮 Geplant:**
* `indexd/` → SQLite + **`trait VectorStore`**: backends `tantivy+hnsw` | `qdrant` **(P2)**
* `llm/` (llama.cpp-Binding, Token-Budget, Prompt-Cache) **(P1)**
* `asr/`, `tts/`, `audio/` (PipeWire-Profile via `profiles.yaml` + CLI-Fassade) **(P1-P2)**
* `commentary/` (Live-Kommentare, Δ-Heuristik) **(P2)**
* `bridge/` (JetStream + GeoJSON, Weltgewebe-Integration) **(P3)**
* `observability/` (erweiterte Metriken, GPU-Tracking, Budget-Guards) **(P2)**
* `adapters/*` (optional, **Wasm-Sandbox**, per Feature-Flag) **(P2)**

---

## 3) APIs & CLI

**APIs** (axum, OpenAI-kompatibel):

* `POST /v1/chat` (Stub, liefert `501` bis zur LLM-Anbindung), `POST /v1/embeddings` *(geplant)*
* `POST /asr/transcribe`, `/obsidian/canvas/suggest`, `/code/pr/draft`
* `GET /health`, `/metrics` (Prometheus)

**CLI (Auszug)**

```
hauski models pull llama3.1-8b-q4
hauski asr transcribe in.wav --model medium --out out.srt
hauski obsidian link --vault ~/Vault --auto
hauski pr draft --repo ~/weltgewebe-repo
hauski comm mail sync --inbox
hauski policy set routing.local_required=true
hauski audio profile set hifi-qobuz
```

---

## 4) Betrieb & Qualität

### 4.1 KPIs (harte Budgets)

* **LLM p95** < 400 ms (8B, kurze Antworten)
* **Index Top-k 20** < 60 ms lokal
* **ASR WER** ≤ 10 % (Studio)
* **GPU-Thermik** < 80 °C, dGPU ≤ 220 W (ASR/LLM-Mix)
* **Kommentar-Signal** ≥ 80 % nicht weggeklickt

### 4.2 Risiken & Abwehr

* Over-Notification → Δ-Schwelle, Cooldowns, Quiet-Mode
* Modell-Drift → versionierte `policies/` + `models.yml`, Canary-Rollouts
* Thermik/Energie → Scheduler, Nacht-Batches, Power-Limits
* API-Brüche (Signal/Telegram) → Adapter hinter Trait, e2e-Tests isoliert, **Wasm-Default**

---

## 5) Sicherheit & Sandbox (konkret)

* **Egress**: deny-by-default, Whitelist per `policies/routing.yaml`
* **Runner-Isolation**: eigene cgroup/Namespace; riskante Adapter **nur** in Wasm (Capability-Filter)
* **Audits**: signierte JSON-Lines (Hash-Kette), `audit verify`
* **Lieferkette**: SBOM (Syft) + cosign-Signierung

---

## 6) Roadmap

* **P1 (jetzt)**: Core+LLM+ASR+TTS · Whisper→Obsidian-Auto-Links · PR-Drafter · Audio-Profile · No-Leak-Guard · TUI Basis · **Budget-Guards**
* **P2 (6–10 Wo.)**: Memory-Schichten · Graph-Lint & Canvas-Diff · On-the-fly-Kommentar · Mail/Matrix · RSS · **GPU-Power-Caps**
* **P3**: Bridge (GeoJSON/NATS) · CI/CD-Kurator · Luthier-Agent · Qdrant-Option · Wake-Word v2 (personenabhängig)

---

## 7) Konfiguration (Beispiele)

**`models.yml`**

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

**`policies/routing.yaml`**

```yaml
egress:
  default: deny
  allow:
    - https://api.matrix.example
routing:
  prefer_local: true
  quality_target: balanced
  cloud_fallback:
    enabled: false
    only_if:
      - "task == 'ocr' && size_mb > 200"
```

**`audio/profiles.yaml`**

```yaml
profiles:
  hifi-qobuz:
    samplerate: 96k
    loopback: false
    loudness: off
  unterricht:
    samplerate: 48k
    loopback: true
    mic_gain_db: 6
  aufnahme:
    samplerate: 48k
    latency_ms: 5
    loopback: false
```

---

## Essenz (verdichtet)

Alles Rust im Hot-Path, **Offline-Default**, **VectorStore als Trait**, **Adapter in Wasm**, **Audio via Profile**, **harte Budgets** in Code+CI. Schnell, robust, erweiterbar.

## Ironische Auslassung

Wenn PipeWire wieder Launen hat, übernimmt die CLI – und Piper spricht seelenruhig weiter.

## Gewissheitsanalyse (∴fore)

* **Unsicherheitsgrad:** ◐ niedrig–mittel
* **Ursachen:** PipeWire-APIs, FFI-Release-Drift (llama/whisper/piper), Messenger-API-Volatilität
* **Meta-Reflexion:** Produktiv/systembedingt – Kern stabil, Ränder beweglich; Budgets+Sandbox begrenzen Schäden.

## Leitfragen

* *War dies die kritischstmögliche Erörterung?*
  Fast.
  **Kontrastvektor:** Eine strikt „candle-only“ LLM-Variante wäre puristischer, aktuell jedoch latenz- und reife-kritisch.
  **Negationsprojektion:** Härteste Gegenposition: „Alles in Docker-Microservices, Hot-Path egal, Hauptsache Features.“ – bricht mit *Performanz > Zukunft > WG-Kompatibilität*.
  **Auditmarker:** Der **Wasm-Default** für Adapter ist kompromisslos – bewusst gewählt zugunsten Supply-Chain-Sicherheit trotz kleinem Overhead.
