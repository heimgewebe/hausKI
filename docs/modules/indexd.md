# Modul: indexd

**Rolle:** Langzeitgedächtnis (episodisch, semantisch)
**Komponente:** `hauski-indexd` (Crate)

---

## Überblick

`indexd` implementiert die Indexierungs- und Query-Schicht von hausKI als **persistentes semantisches Gedächtnis**.
Zentral ist das **`VectorStore`-Trait**, das abstrakte Such- und Embedding-Backends erlaubt (z. B. *tantivy+hnsw* oder *Qdrant*).

## Abgrenzung zu Memory

| Aspekt | Memory (Arbeitsgedächtnis) | indexd (Langzeitgedächtnis) |
|--------|----------------------------|------------------------------|
| **Persistenz** | SQLite K/V | SQLite + Vektoren |
| **Lebensdauer** | TTL-basiert (Sekunden bis Minuten) | Persistent, episodisch |
| **Datentyp** | Key/Value (Bytes) | Dokumente + Embeddings + Metadaten |
| **Zugriff** | Direkt per Key | Semantische Suche, Namespace-Filter |
| **Anwendung** | Session-State, kurzfristige Flags | Chronik, OS-Kontext, Code-Snippets, Insights |

### Hauptaufgaben
- Speichern von Dokument-Embeddings (Text, OS-Kontext, Memory-Snippets)
- Durchführen semantischer Queries (Top-k, Score, Namespace-Filter)
- Bereitstellen der Index-Metriken für `/metrics`

### Namespace-Konventionen

indexd nutzt Namespaces zur semantischen Trennung verschiedener Datenquellen:

| Namespace | Beschreibung | Beispiel-Inhalte |
|-----------|--------------|------------------|
| `chronik` | Ereignis-Historie aus OS/App-Events | System-Events, User-Actions |
| `osctx` | Betriebssystem-Kontext | Prozesse, Netzwerk, Hardware-State |
| `code` | Code-Snippets und Entwickler-Artefakte | Funktionen, Klassen, Commits |
| `docs` | Dokumentation und Wissensartefakte | Markdown, PDFs, API-Docs |
| `insights` | Generierte Insights und Metawissen | Analyse-Ergebnisse, Zusammenfassungen |
| `default` | Fallback für unspezifizierte Inhalte | Allgemeine Einträge |

Alle Namespaces werden normalisiert (getrimmt, Fallback zu `default` bei leer/whitespace).

---

## Architektur

| Komponente | Beschreibung |
|-------------|--------------|
| **Indexer** | wandelt Events/Texts in Embeddings um (via `semantAH`) |
| **Store** | persistiert Embeddings (SQLite oder remote Vector-DB) |
| **API** | REST-Endpunkte `/index`, `/query`, `/related` |

### Provenance Tracking (source_ref)

Dokumente können eine strukturierte Herkunftsreferenz (`SourceRef`) enthalten:

```rust
pub struct SourceRef {
    pub origin: String,   // "chronik", "osctx", "code", "docs", "insights"
    pub id: String,       // event_id, file path, hash
    pub offset: Option<String>, // "line:42", "byte:1337-2048"
}
```

**Konventionen:**
- `origin`: Quell-Namespace (chronik, osctx, code, docs, insights)
- `id`: Eindeutige Referenz (Event-ID, Dateipfad, Commit-Hash)
- `offset`: Position innerhalb der Quelle (Zeile, Byte-Range)
  - ✅ Korrekt: `"line:42"`, `"byte:1337-2048"`, `"offset:123"`
  - ❌ Falsch: Dateipfade gehören nach `id`, nicht nach `offset`

Beispiele:
```json
// Event aus Chronik-Log
{
  "origin": "chronik",
  "id": "/var/log/events/2024-01-01.log",
  "offset": "line:42"
}

// Code-Snippet
{
  "origin": "code",
  "id": "src/main.rs",
  "offset": "line:100-120"
}

// Dokument ohne Positions-Info
{
  "origin": "docs",
  "id": "README.md",
  "offset": null
}
```

---

## Konfiguration

```yaml
index:
  backend: "sqlite"
  path: "~/.hauski/index.db"
  embedding_model: "all-MiniLM-L6-v2"
  max_k: 100
```

---

## Metriken & Budgets

- `index_queries_total` – Gesamtzahl aller Index-Anfragen (inkl. /search, /related)
- `index_query_duration_seconds` – Latenzverteilung der Anfragen
  *Budget:* p95 ≤ 60 ms (konfigurierbar über Limits)

### Budget-Leitplanke

Das System nutzt ein latenzbasiertes Budget:
- Bei Überschreitung des Budgets (> 60 ms p95) sollten Degradations-Maßnahmen greifen
- Aktuelle Implementierung: Warnung im Log, keine automatische Degradation
- Zukünftig: Reduzierung von k, einfachere Filter, Caching

### API-Endpunkte

| Endpoint | Methode | Beschreibung |
|----------|---------|--------------|
| `/index/upsert` | POST | Dokument-Chunks mit Embeddings registrieren |
| `/index/search` | POST | Semantische Suche mit Top-k und Namespace-Filter |
| `/index/related` | POST | Ähnliche Dokumente zu einem gegebenen doc_id finden |
| `/index/stats` | GET | Statistiken über den Index (Dokumente, Chunks, Namespaces) |
| `/index/forget` | POST | Policy-gesteuertes Vergessen von Dokumenten (Admin-Scope) |
| `/index/retention` | GET | Aktive Retention-Policies anzeigen |
| `/index/decay/preview` | POST | Dry-Run: Score-Decay simulieren ohne Änderungen |

---

## Vergessen, Decay & semantische Hygiene

**Konzept:** Ein Gedächtnis ohne Vergessen wird zur Datenkippe. indexd implementiert kontrolliertes, policy-gesteuertes Vergessen zur Vermeidung von semantischer Drift und Bedeutungsüberlagerung.

### Vergessensmodi

indexd unterstützt vier explizite Modi des Vergessens:

#### 1. Zeitliches Vergessen (Time-Decay)

Ältere Einträge verlieren kontinuierlich an Relevanz.

**Mechanismus:**
- Jeder Eintrag hat ein `ingested_at`-Timestamp
- Optional: `half_life` (in Sekunden) pro Namespace oder Document
- Score-Berechnung: `final_score = similarity_score × decay_factor`
- Decay-Faktor: `decay_factor = 0.5 ^ (age_seconds / half_life)`

**Beispiel:**
```yaml
# policies/indexd_retention.yaml
namespaces:
  chronik:
    half_life_seconds: 2592000  # 30 Tage
  osctx:
    half_life_seconds: 86400    # 1 Tag
  code:
    half_life_seconds: null     # Kein Decay
```

**Eigenschaften:**
- Kontinuierlicher, deterministischer Relevanzverlust
- Keine harten Löschungen – nur Score-Reduktion
- Semantisch relevante alte Einträge können durch hohe similarity_score überleben

#### 2. Namespace-Retention (Strukturelles Vergessen)

Pro Namespace konfigurierbare Limits und Purge-Strategien.

**Konfiguration:**
```yaml
namespaces:
  chronik:
    max_items: 10000
    max_age_seconds: 7776000  # 90 Tage
    purge_strategy: oldest     # oldest | lowest_score
  default:
    max_items: null            # Unbegrenzt
    max_age_seconds: null
    purge_strategy: null
```

**Purge-Strategien:**
- `oldest`: Älteste Einträge zuerst (FIFO)
- `lowest_score`: Niedrigste kombinierte Scores (Decay + Relevanz)
- `random`: **VERBOTEN** – keine zufälligen Löschungen

**Triggering:**
- Automatisch bei Überschreitung von `max_items` oder `max_age_seconds`
- Nur bei `/upsert`-Operationen (niemals implizit bei Queries)

#### 3. Intentional Forget (Policy-Entscheid)

Explizite Löschung durch Policy-gesteuerte Events.

**API:**
```http
POST /index/forget
Content-Type: application/json

{
  "filter": {
    "namespace": "chronik",
    "older_than": "2024-01-01T00:00:00Z",
    "source_ref_origin": "osctx"
  },
  "reason": "Manual cleanup after system migration",
  "confirm": true
}
```

**Sicherheitsgeländer:**
- Erfordert `confirm: true` im Request-Body
- Keine globalen Löschungen ohne Filter
- Erzeugt strukturierte Logs + Metriken
- Dry-Run via `/index/forget?dry_run=true`

#### 4. Semantisches Vergessen (Relevanzabnahme)

**Status:** Geplant (nicht in v0.1)

Dokumente mit dauerhaft niedrigen Scores werden als irrelevant markiert und priorisiert vergessen.

---

### Metriken für Vergessen

indexd exportiert folgende Observability-Metriken:

| Metrik | Typ | Beschreibung |
|--------|-----|--------------|
| `index_items_total{namespace}` | Gauge | Aktuelle Anzahl Dokumente pro Namespace |
| `index_items_forgotten_total{namespace,reason}` | Counter | Gelöschte Dokumente (Grund: ttl, retention, manual) |
| `index_decay_applied_total` | Counter | Anzahl Score-Decay-Berechnungen |
| `index_retention_purges_total{namespace,strategy}` | Counter | Ausgeführte Retention-Purges |

**Verwendung:**
```promql
# Vergessensrate pro Namespace
rate(index_items_forgotten_total[5m])

# Anteil Decay-betroffener Dokumente
index_decay_applied_total / index_items_total
```

---

### Sicherheitsrichtlinien

**Verboten:**
- ❌ Implizites Vergessen (z. B. bei Index-Rebuild)
- ❌ Globales `DELETE *` ohne Filter
- ❌ Zufällige Purge-Strategien (`random`)
- ❌ Stilles Vergessen ohne Logs/Metriken

**Pflicht:**
- ✅ Alle Löschungen erzeugen Metriken
- ✅ Intentional Forget erfordert `reason`-String
- ✅ Dry-Run-Modus für alle Purge-Operationen
- ✅ Vergessen ist beobachtbar (Logs, Metrics, Events)

---

## Offene Aufgaben

- [ ] SQLite-Persistenz implementieren (aktuell nur In-Memory)
- [ ] HNSW-Backend für echte Vektor-Ähnlichkeitssuche
- [ ] Beispiel-Querys in Dokumentation ergänzen
- [ ] API-Spec per `utoipa` exportieren
- [ ] Semantisches Vergessen (Relevanzabnahme) implementieren

## Status

**Implementiert:**
- ✅ In-Memory-Store mit Namespace-Support
- ✅ Substring-basierte Textsuche
- ✅ Metadaten (source_ref, ingested_at)
- ✅ /upsert, /search, /related, /stats Endpoints
- ✅ Metriken-Integration

**In Entwicklung:**
- 🔄 SQLite-Persistenz
- 🔄 Vektor-Embeddings und HNSW-Index
- 🔄 Time-Decay und Retention-Policies
- 🔄 Forget-API und Dry-Run-Modus
