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

---

## Offene Aufgaben

- [ ] SQLite-Persistenz implementieren (aktuell nur In-Memory)
- [ ] HNSW-Backend für echte Vektor-Ähnlichkeitssuche
- [ ] Beispiel-Querys in Dokumentation ergänzen
- [ ] API-Spec per `utoipa` exportieren

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

---

**Letzte Aktualisierung:** 2025-10-23
