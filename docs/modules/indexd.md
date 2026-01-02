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

- `index_queries_total`
- `index_query_duration_seconds`
  *Budget:* p95 ≤ 60 ms

---

## Offene Aufgaben

- [ ] HNSW-Backend dokumentieren
- [ ] Beispiel-Querys ergänzen
- [ ] API-Spec per `utoipa` exportieren

---

**Letzte Aktualisierung:** 2025-10-23
