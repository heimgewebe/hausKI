# Ist-Stand vs. Roadmap – HausKI Implementierungsstatus

Dieses Dokument unterscheidet klar zwischen **aktuell implementierten Funktionen** (Ist-Stand)
und **geplanten Erweiterungen** (Roadmap). Es dient als Orientierung für Entwickler und Nutzer,
um realistische Erwartungen an den aktuellen Funktionsumfang zu setzen.

**Letzte Aktualisierung:** 2025-12-03

---

## 1. Indexierung (`indexd`)

### ✅ Ist-Stand (Implementiert)

- **In-Memory-Indexierung** über `HashMap` (`IndexState`)
- **Substring-basierte Suche** mit einfachem Scoring (`substring_match_score`)
- **Namespace-Unterstützung** für logische Trennung von Dokumenten
- **HTTP-API** mit `/index/upsert` und `/index/search` Endpoints
- **Metriken** für Latency-Tracking und Budget-Überwachung
- **Tests** für Basis-Funktionalität (Upsert, Search, Namespace-Handling)

**Einschränkungen:**
- Keine Persistenz: Daten gehen bei Neustart verloren
- Keine Vektorsuche: nur textueller Substring-Match
- Keine skalierbare Index-Struktur (tantivy, HNSW)

### 🔮 Roadmap (Geplant)

**P2 (Mittelfristig):**
- `VectorStore`-Trait als abstrakte Schnittstelle
- SQLite-Backend für persistente Metadaten
- Tantivy+HNSW als Default-Backend für Vektorsuche
- Embedding-basierte semantische Suche

**P3 (Langfristig):**
- Qdrant als optionales Feature für skalierbare Deployments
- Hybrid-Suche (Vektor + Volltext kombiniert)
- Index-Sharding für größere Datenmengen

---

## 2. Module: LLM, ASR, TTS, Audio

### ✅ Ist-Stand (Implementiert)

**Keine dieser Module sind aktuell in Rust implementiert.**

Die folgenden Crates existieren **nicht** unter `crates/`:
- `llm/` (llama.cpp Bindings)
- `asr/` (whisper-rs)
- `tts/` (piper-rs)
- `audio/` (PipeWire-Profile, Audio-Pipeline)

**Workaround:** Externe Services können via HTTP-Upstream angebunden werden
(z. B. `HAUSKI_CHAT_UPSTREAM_URL` für LLM-Chat).

### 🔮 Roadmap (Geplant)

**P1 (Hohe Priorität):**
- `llm/`: llama.cpp FFI-Binding für lokale Inference
- `asr/`: whisper-rs Integration für Spracherkennung
- `tts/`: piper-rs Integration für Text-to-Speech

**P2 (Mittelfristig):**
- `audio/`: PipeWire-Abstraktion mit `profiles.yaml`
- Audio-Pipeline für Aufnahme, Loopback, und Verarbeitung

**P3 (Langfristig):**
- Modell-Hot-Swapping ohne Restart
- Wake-Word-Detection (optional, prozess-separiert)
- Luthier-Agent (Akkord/Tempo-Analyse)

---

## 3. Plugins & Cloud-Fallback (Core)

### ✅ Ist-Stand (Implementiert)

- `plugin_routes()` und `cloud_routes()` existieren als **leere Platzhalter**
- Funktionen liefern `Router::new()` zurück (keine Routen registriert)
- **safe_mode** Feature-Flag: deaktiviert Plugin/Cloud-Routen wenn gesetzt

**Einschränkungen:**
- Keine Plugin-Schnittstelle verfügbar
- Keine Cloud-Fallback-Logik implementiert
- HTTP-Requests an diese Routen führen zu `404 Not Found`

### 🔮 Roadmap (Geplant)

**P2 (Mittelfristig):**
- **Plugin-Schnittstelle:**
  - `/plugins/:name/invoke` Endpoint
  - Plugin-Registry für Wasm-Module (wasmtime)
  - Capability-basierte Sandbox (Datei-, Netzwerk-, GPU-Zugriff)
- **Cloud-Fallback:**
  - `/cloud/fallback` Endpoint mit Policy-basiertem Routing
  - Egress-Guard Integration (Whitelist-Check)
  - Konfigurierbarer Upstream (z. B. OpenAI, Anthropic)

**P3 (Langfristig):**
- Plugin-Hot-Reload ohne Server-Neustart
- Multi-Cloud-Routing mit Cost-Optimierung
- Plugin-Marketplace mit Signatur-Verifikation

---

## 4. Heimlern (Policy-Learning via Bandits)

### ✅ Ist-Stand (Implementiert)

- `heimlern-core` und `heimlern-bandits` sind im Workspace verfügbar (`vendor/`)
- **Optional** in `hauski-policy-api` via Feature `heimlern`
- **Nicht** in `hauski-core` integriert
- Keine aktive Nutzung im Hauptserver

**Einschränkungen:**
- Keine adaptive Policy-Entscheidung in Produktion
- Bandits-Logik ist experimentell und deaktiviert

### 🔮 Roadmap (Geplant)

**P2 (Mittelfristig):**
- Integration in `PolicyEngine` als optionaler Hook
- Messbare Use-Cases:
  - Routing-Entscheidung lokal vs. Cloud
  - Model-Selection (8B vs. 70B basierend auf Task-Komplexität)
- Feature-Flag: `HAUSKI_ENABLE_HEIMLERN=true`

**P3 (Langfristig):**
- Multi-Armed-Bandit für API-Endpunkt-Auswahl
- Contextual Bandits mit Request-Features (User, Task-Typ, Latenz-Historie)
- A/B-Testing-Framework für neue Policies

---

## 5. Memory-System

### ✅ Ist-Stand (Implementiert)

- **SQLite-basierter Key-Value-Store** (`crates/memory`)
- **TTL-Unterstützung** für automatisches Ablaufen von Einträgen
- **Pin/Unpin-Mechanismus** zum Schutz vor Eviction
- **Janitor-Task** für periodische Bereinigung abgelaufener Einträge
- **HTTP-API:** `/memory/get`, `/memory/set`, `/memory/evict`
- **Prometheus-Metriken:** `memory_items_pinned`, `memory_evictions_total`

**Einschränkungen:**
- Keine expliziten Memory-Schichten (`short_term`, `working_context`, `long_term`)
- Keine semantische Verknüpfung oder Retrieval-Policies

### 🔮 Roadmap (Geplant)

**P2 (Mittelfristig):**
- Memory-Schichten mit unterschiedlichen TTL-Defaults
- Themen-Buckets für logische Gruppierung
- Retrieval-Policies (LRU, Priority-basiert)

**P3 (Langfristig):**
- Konflikt-Detektor für widersprüchliche Einträge
- Automatische Backlinks zu Obsidian/Canvas
- Memory-Snapshots und Restore-Funktionalität

---

## 6. Sicherheit & Egress-Control

### ✅ Ist-Stand (Implementiert)

- **Egress-Guard** mit Whitelist-Validierung (`crates/core`)
- **allowlisted_client()** für sichere HTTP-Requests
- **CORS-Middleware** mit konfigurierbarer Origin-Kontrolle
- **Request-Guards:** Timeout (1500ms) und Concurrency-Limit (512)

**Einschränkungen:**
- Keine systemd-cgroup/Namespace-Isolation aktiv
- Keine Wasm-Sandbox für Plugins (da Plugins nicht implementiert)
- Kein KMS oder rage/age-Integration

### 🔮 Roadmap (Geplant)

**P1 (Hohe Priorität):**
- systemd-Slices für Resource-Limits (CPU, Mem, IO)
- rage (age) für Verschlüsselung sensibler Konfiguration

**P2 (Mittelfristig):**
- Wasm-Sandbox (wasmtime) für Plugin-Ausführung
- Audit-Log mit signierten JSON-Lines (Hash-Kette)
- RBAC (Admin, Operator, Gast)

**P3 (Langfristig):**
- SBOM-Generierung (Syft) und Signierung (cosign)
- Automatische Secret-Rotation
- Supply-Chain-Verifikation in CI

---

## 7. Observability & Metriken

### ✅ Ist-Stand (Implementiert)

- **Prometheus-Exporter** unter `/metrics`
- **HTTP-Request-Metriken:** Counter, Latency-Histogramme
- **Budget-Tracking** für Index-Latenz
- **Memory-Metriken:** Pinned/Unpinned-Items, Evictions
- **Health-Checks:** `/health`, `/healthz`, `/ready`

**Einschränkungen:**
- Keine GPU-Metriken (VRAM, Thermik, Power)
- Keine automatischen Budget-Gates (nur Logging)
- Keine OpenTelemetry-Integration

### 🔮 Roadmap (Geplant)

**P2 (Mittelfristig):**
- GPU-Metriken via `nvidia-smi` Hook
- Budget-Gates als CI-Schritt (Performance-Regression-Tests)
- Strukturiertes Tracing mit `tracing-opentelemetry`

**P3 (Langfristig):**
- Grafana-Dashboard-Templates
- Alert-Manager-Integration (Slack, PagerDuty)
- Distributed Tracing für Multi-Service-Deployments

---

## 8. CLI & UX

### ✅ Ist-Stand (Implementiert)

- **CLI-Framework** mit clap (`crates/cli`)
- **Basis-Kommandos:** `hauski serve`, `hauski config`
- **Konfigurations-Loader** für `models.yml`, `limits.yaml`, `routing.yaml`

**Einschränkungen:**
- Keine Audio-Profile-Kommandos
- Keine Obsidian/Canvas-Integration
- Keine PR-Drafter oder Luthier-Tools

### 🔮 Roadmap (Geplant)

**P1 (Hohe Priorität):**
- `hauski models pull <model-id>`
- `hauski asr transcribe <file>`

**P2 (Mittelfristig):**
- `hauski obsidian link --vault <path>`
- `hauski pr draft --repo <path>`
- `hauski audio profile set <name>`

**P3 (Langfristig):**
- TUI mit ratatui (Status-Dashboard, Logs, Metriken)
- VS-Code-Extension (PR-Panel, Inline-Hints)
- Obsidian-Plugin (Canvas-Suggest, Auto-Links)

---

## Zusammenfassung

| Feature | Status | Priorität |
|---------|--------|-----------|
| **Indexd (In-Memory)** | ✅ Implementiert | - |
| **Indexd (SQLite + Vektor)** | 🔮 Geplant | P2 |
| **LLM/ASR/TTS/Audio** | 🔮 Geplant | P1 |
| **Plugins & Cloud-Fallback** | 🔮 Geplant | P2 |
| **Heimlern Integration** | 🔮 Geplant | P2 |
| **Memory-System (Basis)** | ✅ Implementiert | - |
| **Memory-Schichten** | 🔮 Geplant | P2 |
| **Egress-Guard** | ✅ Implementiert | - |
| **Wasm-Sandbox** | 🔮 Geplant | P2 |
| **Prometheus-Metriken** | ✅ Implementiert | - |
| **GPU-Metriken** | 🔮 Geplant | P2 |
| **CLI (Basis)** | ✅ Implementiert | - |
| **TUI & Extensions** | 🔮 Geplant | P3 |

**Legende:**
- ✅ **Implementiert:** Code existiert, Tests laufen, produktiv nutzbar
- 🔮 **Geplant:** Spezifiziert, aber noch nicht implementiert
- **P1:** Hohe Priorität (nächste 1-2 Monate)
- **P2:** Mittelfristig (3-6 Monate)
- **P3:** Langfristig (6+ Monate)

---

## Referenzen

- [`docs/inconsistencies.md`](./inconsistencies.md) – Detaillierte Analyse der Abweichungen
- [`hauski-skizze.md`](./hauski-skizze.md) – Architektur-Vision (Roadmap-fokussiert)
- [`hauski-stack.md`](./hauski-stack.md) – Technologie-Stack (Mix aus Ist/Roadmap)
- [`CONTRIBUTING.md`](./CONTRIBUTING.md) – Beitragsrichtlinien und DoD
