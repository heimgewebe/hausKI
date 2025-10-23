# Modul: indexd

**Rolle:** Speicherung und semantische Suche  
**Komponente:** `hauski-indexd` (Crate)

---

## Überblick

`indexd` implementiert die Indexierungs- und Query-Schicht von hausKI.
Zentral ist das **`VectorStore`-Trait**, das abstrakte Such- und Embedding-Backends erlaubt (z. B. *tantivy+hnsw* oder *Qdrant*).

### Hauptaufgaben
- Speichern von Dokument-Embeddings (Text, OS-Kontext, Memory-Snippets)
- Durchführen semantischer Queries (Top-k, Score, Namespace-Filter)
- Bereitstellen der Index-Metriken für `/metrics`

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
