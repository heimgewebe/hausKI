# Intent Resolver

Der `intent-resolver` analysiert den Kontext (Git, GitHub Events), um die Absicht der aktuellen Änderung zu bestimmen. Dies hilft `hausKI` zu entscheiden, welche Tools oder Prozesse verwendet werden sollen (z.B. Coding vs. Writing vs. CI Triage).

## Funktionsweise

Der Resolver sammelt Signale aus:
1.  **Geänderten Dateien**: Pfade und Dateiendungen.
2.  **Workflow-Namen**: Wenn in CI ausgeführt.
3.  **PR-Kommentaren**: Spezielle Befehle wie `/quick` oder `/review`.

Basierend auf diesen Signalen wird ein `Intent` mit einer `confidence` (0.0 - 1.0) berechnet.

### Intent-Typen

*   `coding`: Änderungen an Quellcode (`src/`, `crates/`, `.rs`, `.py` etc.).
*   `writing`: Änderungen an Dokumentation (`docs/`, `*.md`).
*   `ci_triage`: Änderungen an CI-Konfigurationen (`.github/workflows/`) oder getriggert durch PR-Kommentare.
*   `contracts_work`: Änderungen an Contracts (`contracts/`).
*   `unknown`: Keine klaren Signale.

### Confidence Berechnung

*   Basis: 0.55
*   +0.15: Starke Pfad-Signale (> 80% der Änderungen deuten auf einen Typ hin).
*   -0.20: Gemischte/unklare Signale (< 60% Dominanz).
*   Clamp: 0.0 - 1.0

## Verwendung

### CLI

```bash
hauski intent
```

Ausgabe (Beispiel):

```json
{
  "intent": "coding",
  "confidence": 0.7,
  "signals": [
    {
      "kind": "changed_path",
      "ref": "crates/core/src/lib.rs",
      "weight": 0.9
    }
  ],
  "created_at": "2023-10-27T10:00:00Z",
  "context": {}
}
```

Mit Datei-Output:

```bash
hauski intent --output out/intents/intent-current.json
```

### CI Integration

In GitHub Actions kann der Intent Resolver genutzt werden, um nachfolgende Schritte zu steuern.

```yaml
- name: Determine Intent
  run: |
    cargo run -p hauski-cli -- intent --output intent.json
    echo "INTENT=$(jq -r .intent intent.json)" >> $GITHUB_ENV
```
