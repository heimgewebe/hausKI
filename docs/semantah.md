# semantAH – Semantische Gedächtnisschicht für HausKI

semantAH erweitert HausKI um ein langlebiges semantisches Gedächtnis. Die Pipelines erzeugen Embeddings, bauen einen Index, verbinden Notizen als Graph und schreiben "Related"-Blöcke in Obsidian-Dateien. Dieser Bootstrap umfasst Dokumentation, Konfiguration, Skripte und Rust-Scaffolds, damit du sofort lokal experimentieren kannst.

## Überblick: Index → Graph → Related

| Stufe | Zweck | Artefakte | Hinweise |
|-------|-------|-----------|----------|
| **Index** | Chunked Markdown, Canvas und Events in einen semantischen Index kippen | `.gewebe/embeddings.parquet` | Embeddings werden aus Text-Schnipseln erzeugt; Metadaten (Doc-ID, Namespace) gehen als JSON in dasselbe Verzeichnis. |
| **Graph** | Ähnliche Knoten verbinden, um Pfade zwischen Notizen sichtbar zu machen | `.gewebe/nodes.jsonl`, `.gewebe/edges.jsonl` | Nodes enthalten Markdown- und Canvas-IDs samt Kontext. Edges tragen Scores und Cutoffs. |
| **Related** | Obsidian-Blocks aktualisieren, Canvas-Anmerkungen schreiben, Reports erzeugen | `.gewebe/reports/*.md`, aktualisierte Markdown-Dateien | Related-Blöcke in Markdown folgen dem Marker `<!-- related:auto:start --> … <!-- related:auto:end -->`. |

### Obsidian-Related-Block

semantAH aktualisiert Related-Abschnitte in Notizen idempotent. Der Marker wird immer vollständig ersetzt:

```
<!-- related:auto:start -->
- auto (score ≥ 0.80)
- review (0.70 – 0.79)
<!-- related:auto:end -->
```

Die Skripte schreiben automatisch alle Treffer mit Score ≥ 0.80. Funde zwischen 0.70 und 0.79 landen im Review-Abschnitt. Unterhalb von 0.70 werden keine Einträge erzeugt.

## Betriebsmodi

semantAH unterstützt zwei Betriebsmodi, die sich gegenseitig ergänzen:

1. **Rust-Dienst**
   - axum-Service unter `/index`.
   - Endpunkte `POST /index/upsert` und `POST /index/search` akzeptieren JSON-Schemata gemäß Abschnitt [HTTP-Schnittstellen](#http-schnittstellen).
   - `/metrics` exportiert Prometheus-Kennzahlen inkl. p95-Latenzen.
2. **Skript-Pipeline („Classic“)**
   - Python-Skripte unter `tools/semantah/` orchestriert durch ein `Makefile` und WGX-Recipes.
   - Optionaler systemd-Timer (User-Unit) startet zyklisch `make related`, um Notizen aktuell zu halten.

## Artefakt-Pfade

Alle semantAH-Artefakte liegen standardmäßig unter `$HOME/.local/state/hauski/index`. Innerhalb dieses Verzeichnisses werden Unterordner nach Namespace angelegt, z. B. `default`, `obsidian`, `events`.

## KPIs & Budgets

- **Performanceziel:** `top-k=20` Suchanfragen müssen unter 60 ms p95 bleiben.
- **Metriken:**
  - `http_requests_total{method,path,status}`
  - `http_request_duration_seconds_bucket{method,path,le}`
  - `http_request_duration_seconds_sum`
  - `http_request_duration_seconds_count`
  - `build_info{service="indexd"}`
- Die CI-Stufe `index-budget-gate` prüft perspektivisch, dass das Budget eingehalten wird. Derzeit hinterlegt sie einen TODO-Hinweis.

## HTTP-Schnittstellen

Alle JSON-Beispiele zeigen den erwarteten Body und die Antwortstruktur. Die Implementierung speichert noch in-memory, liefert aber bereits korrekte Statuscodes.

### `POST /index/upsert`

```json
{
  "doc_id": "docs/semantah",
  "namespace": "default",
  "chunks": [
    {
      "chunk_id": "docs/semantah#intro",
      "text": "Chunkierte Inhalte …",
      "embedding": [0.0, 0.1, 0.2],
      "meta": {"source": "markdown"}
    }
  ],
  "meta": {
    "kind": "markdown",
    "path": "docs/semantah.md"
  }
}
```

Antwort:

```json
{
  "status": "queued",
  "ingested": 1
}
```

### `POST /index/search`

```json
{
  "query": "Was ist semantAH?",
  "k": 20,
  "namespace": "default"
}
```

Antwort (Stub):

```json
{
  "matches": [],
  "latency_ms": 0.0,
  "budget_ms": 60
}
```

## Skript-Pipeline

Die Classic-Pipeline lebt unter `tools/semantah/`:

- `build_index.py`: Parst Notizen, zerlegt sie in Chunks, ruft den Embedder (derzeit TODO) und schreibt `embeddings.parquet`.
- `build_graph.py`: Liest Embeddings, berechnet Ähnlichkeiten und schreibt `nodes.jsonl` sowie `edges.jsonl`.
- `update_related.py`: Aktualisiert Related-Blöcke in Markdown-Dateien. Unterstützt `--check` für einen Dry-Run (nur Diff-Ausgabe).
- `Makefile`: Targets `all`, `index`, `graph`, `related`, `clean` orchestrieren die Schritte. `wgx run semantah:all` ruft `make -C tools/semantah all` auf.

### systemd (optional)

Für automatisierte Aktualisierungen kannst du user-scoped systemd-Units verwenden. Beispiel unter `$HOME/.config/systemd/user/`:

`hauski-semantah.service`
```
[Unit]
Description=HausKI semantAH Pipeline
After=network.target

[Service]
Type=oneshot
WorkingDirectory=%h/.local/state/hauski/index
ExecStart=%h/.cargo/bin/wgx run semantah:related
Environment=RUST_LOG=info

[Install]
WantedBy=default.target
```

`hauski-semantah.timer`
```
[Unit]
Description=HausKI semantAH Pipeline Timer

[Timer]
OnCalendar=hourly
Persistent=true

[Install]
WantedBy=timers.target
```

Aktiviere die Timer-Unit mit:

```bash
systemctl --user enable --now hauski-semantah.timer
```

## Weiterführende Ressourcen

- [docs/runbooks/semantah_local.md](runbooks/semantah_local.md) — Quickstart, Ollama-Setup, Seeds.
- `.wgx/profile.yml` — WGX-Recipes (`run semantah:*`, `guard semantah`).
- `configs/hauski.yml` — Standardpfade, Embedder-Konfiguration und Budgets.

## Nächste Schritte

- HTTP-Client für `OllamaEmbedder` implementieren.
- Persistenz für den Index (tantivy oder sqlite) ergänzen.
- Timeline-Integration basierend auf `seeds/semantah/events.sample.jsonl` aufbauen.
- CI-Budget-Gate mit realem vegeta/k6-Lauf unterlegen.
