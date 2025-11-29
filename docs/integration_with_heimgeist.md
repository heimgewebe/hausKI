# Integration mit Heimgeist

HausKI ist die ausführende Intelligenzschicht des Heimgewebes. Er interagiert mit Heimgeist über klar definierte Delegationspfade.

## Delegationsfluss (vereinfacht)

1. Heimgeist erkennt ein Muster oder Risiko.
2. Heimgeist erzeugt eine Delegation:

```json
{
  "target": "hauski",
  "action": "assist",
  "context": {
    "query": "Erkläre PR #17 im Hinblick auf semantischen Drift"
  }
}
```

3. HausKI startet eine Session (`/assist`), führt RAG, Modelle, Tools aus.
4. HausKI liefert Ergebnis zurück oder dokumentiert in `chronik`.
5. Heimgeist bewertet das Ergebnis und integriert es in sein Systemwissen.

---

## Zuständigkeitsabgrenzung

* **Heimgeist:**
    * Systemweite Interpretation
    * Delegationslogik
    * Risiko- und Driftbewertung
* **HausKI:**
    * RAG, Modellinferenz, Audio/Text
    * Konkrete Problemlösung
    * Dev-Assistenz + Tool-Ausführung

---

## Warum diese Trennung?

* Verhindert Überladung (ein Agent für alles ist immer schlecht).
* Entkoppelt Reflexion (Heimgeist) von Interaktion (HausKI).
* Macht das System skaliert wartbar.
