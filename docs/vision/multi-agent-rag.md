# Multi-Agent RAG – Orchestrierung über hausKI

**Ziel:** Spezialisierte Agenten (Code, Knowledge, Research, Music) orchestrieren die bestehenden Bausteine:

## Pfeiler (re-use)
- **hausKI Core**: HTTP/API, `/index/*`, `/ask`, Metriken (Ingress).
- **semantAH**: Embeddings/Index; liefert Chunks & Ähnlichkeitssuche.
- **vault-gewebe**: Obsidian-Vault als Wissensquelle.
- **aussensensor**: externe Feeds (Blogs, arXiv, …).
- **mitschreiber**: OS/Audio/Text-Kontext (Transkripte, Sessions).

## Neuer Layer: Agent-Orchestrierung (aus Template)
- Supervisor (LangGraph) routet zu **Code-Agent** und **Knowledge-Agent**.
- **Tool-Calls** strikt per JSON-Schema (kanonisch `contracts/lenskit/*.schema.json`; Legacy-Kopie unter `contracts/tools/*.schema.json`).
- **RAG-Loop:** Query → semantAH (Top-K) → Synthese → Antwort **mit Zitaten**.

## MVP (Woche 1)
1. **Knowledge-Agent**: Vault + semantAH; Zitate verpflichtend.
2. **Code-Agent**: semantische Codesuche (Start simpel, später AST/Callgraph).
3. **Supervisor (stateless)**: `route_to_specialist(q) → {knowledge|code}`.

## Schnittstellen (Status quo)
- **/index/upsert**: bleibt Quelle für Chunks.
- **/ask**: schnelle Retrieval-Probe (Top-K ≤ 100).

## Geplanter Endpunkt (optional, später)
- **POST `/assist`** `{ "q": "...", "mode": "auto|code|knowledge" }`
  - Response: `{ answer, citations[], trace[], latency_ms }`

## Evaluation
- Qualität: Zitattrace (N Quellen), Coverage, Halluzinationsquote.
- Latenzbudget: p95 Suche+Synthese vs. `policies/limits.yaml`.

## Routing (einfach)
```
if "code|rust|python|crate|fn|module" in q → code_agent
elif "erkläre|was weiß ich|notiz|paper|konzept" in q → knowledge_agent
else → knowledge_agent (default)
```

## Open Points
- AuthZ für externe Fetcher (optional).
- Music-Agent erst ab Phase 3/4.

## Next Steps (konkret)
1) **Templates syncen:** `just agents.sync` (zieht `templates/agent-kit/**` flach ins Repo).
2) **Contracts prüfen:** `docs/contracts/lenskit/*.schema.json` (Legacy-Spiegel unter `docs/contracts/tools/*.schema.json`, siehe unten).
3) **Dry-Run:** `just agents.run` (Supervisor + Dummy-Adapter).
4) **Adapter anschließen:** semantAH-Search + Vault-Reader an Tools hängen.

---

## Tool-Schemas (verlinkte $id aus metarepo)
**search_codebase.schema.json** und **query_vault.schema.json** liegen kanonisch unter `docs/contracts/lenskit/` und referenzieren die `lenskit`-$id aus dem metarepo.

Eine kompatible Legacy-Kopie bleibt unter `docs/contracts/tools/` (historischer Namespace), damit bestehende Consumer nicht brechen. Beide Varianten sind lokal validierbar (AJV).

> Hinweis: Quelle der Wahrheit ist im metarepo; dieses Repo hält nur eine synchronisierte Kopie (siehe `scripts/agents-sync-from-metarepo.sh`).
