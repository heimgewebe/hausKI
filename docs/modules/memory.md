# Memory

Die Memory-Schicht verwaltet Kontextwissen über mehrere Zeithorizonte. Sie ist im Stack als eigener Crate vorgesehen und kapselt Retrieval-Policies gegenüber `core` und `indexd`.

## Designziele

- **Schichtenmodell:** Kurzzeit- („short“), Arbeits- („working“) und Langzeitspeicher („long“) mit separaten TTLs und Pin-Optionen.
- **Policy-gesteuert:** `policies/memory.yaml` definiert Aufbewahrung, Prioritäten und Grenzwerte pro Layer.
- **Hot/Cold-Split:** Relevante Kontexte landen im In-Memory-Index (`indexd`), kalte Daten werden in SQLite oder Files persistiert.
- **Budget-Awareness:** Jeder Abruf respektiert die in `limits.yaml` hinterlegten p95-Vorgaben.

## Schnittstellen (geplant)

| Funktion | Beschreibung |
| --- | --- |
| `record_interaction(event)` | Speichert neue Interaktions- oder Analyseartefakte unter Berücksichtigung der Routing-Policy. |
| `fetch_context(query, budget)` | Liefert Kontext-Chunks für LLM-Aufrufe, inklusive Score und Herkunftslayer. |
| `pin(item_id, layer)` | Markiert Elemente als unantastbar trotz TTL. |
| `expire()` | Hintergrundjob für TTL-Löschungen und Budgetbereinigung. |

## Zusammenspiel mit anderen Modulen

- **Core:** ruft Memory-APIs auf, bevor `/ask` beantwortet wird, und entscheidet anhand der Scores über Prompt-Injektion.
- **Indexd:** dient als schneller Retrieval-Store für „working“-Kontexte; Memory synchronisiert relevante Chunks dorthin.
- **Policies:** Memory respektiert Routing- und Limit-Policies, um Egress- oder Token-Budgets nicht zu verletzen.

## Implementierungsstand

Der Crate ist im Workspace vorgesehen, aber noch nicht implementiert. Die Architektur-Dokumente bilden den Fahrplan – sobald Memory gebaut wird, sollten Unit-Tests für TTL/Pinning sowie Integrationstests gegen `indexd` ergänzt werden.
