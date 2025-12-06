# Playbook: CI-Fehler auf `main` in einem Heimgewebe-Repo

Dieses Playbook beschreibt, wie hausKI auf einen harten CI-Fehler auf `main`
in einem Heimgewebe-Repository reagieren soll.

Ziel:
- Fehler nicht nur „zur Kenntnis nehmen“, sondern
  - systematisch einordnen,
  - automatisch erste Schritte anstoßen,
  - und eine saubere Spur in chronik hinterlassen.

Die YAML-Quelle des Playbooks liegt unter `playbooks/ci-failure-main.yaml`.

---

## 1. Trigger-Bedingung

Das Playbook wird aktiv, wenn ein Event eingeht, das in etwa so aussieht
(vereinfachtes Beispiel im Stil von `event.line`):

```json
{
  "kind": "ci.result",
  "ts": "2025-12-06T20:15:00Z",
  "labels": {
    "repo": "metarepo",
    "branch": "main",
    "status": "failure",
    "provider": "github_actions",
    "workflow": "ci"
  },
  "meta": {
    "run_url": "https://github.com/heimgewebe/metarepo/actions/runs/1234567890"
  }
}
```

Der Prototyp-Trigger ist bewusst eng gefasst:

- `kind == "ci.result"`
- `labels.status == "failure"`
- `labels.branch == "main"`

Später können weitere Branch- oder Schweregrade abgebildet werden.

---

## 2. Beabsichtigte Reaktion (Kurzfassung)

1. **Guard laufen lassen**
   - Schritt: `wgx guard` für das betroffene Repo auf `main` ausführen.
   - Zweck: schnelle, standardisierte Überprüfung der wichtigsten Checks
     (z. B. Toolchain, Contracts, Linting).

2. **Sichter-Quick-Review anstoßen**
   - Schritt: Im zugehörigen PR oder Workflow-Kontext einen Kommentar platzieren:
     - `@heimgewebe/sichter /quick`
   - Zweck: automatisierte KI-Review-Runde, die den Fehler einordnet
     und konkrete Fix-Vorschläge sammelt.

3. **Eintrag in chronik**
   - Schritt: ein neues Event `ci.failure.logged` in chronik schreiben,
     das den Fehler, das Repo und den Link zum Run festhält.
   - Zweck: der Organismus soll sich an wiederkehrende Fehler „erinnern“
     und Langzeitmuster erkennen können (z. B. flakey Tests, instabile Pipelines).

Diese drei Schritte sind im YAML-Playbook als strukturierte Aktionen hinterlegt.

---

## 3. Semantische Leitlinien (Anbindung an die hausKI-Guidance)

Grundprinzipien, die zu den bestehenden `ai_guidance`-Hinweisen passen:

- **Typed I/O**
  - Events sollen dem `event.line`-Contract folgen (bzw. seinen Ableitungen).
  - hausKI sollte Playbooks später möglichst gegen generierte Typen aus den
    Contracts kompilieren, statt ungetypte JSON-Hacks zu verwenden.

- **Tracing-Kontext durchreichen**
  - Jeder Schritt (WGX, Sichter, chronik) sollte einen konsistenten
    Trace-/Correlation-Context tragen (z. B. Run-ID, Event-ID).

- **Keine ad-hoc-Magie**
  - Dieses Playbook ist bewusst explizit: alle Aktionen stehen lesbar in YAML.
  - hausKI ist hier primär Orchestrierer, nicht „black box Magier“.

---

## 4. Ausblick: Wie hausKI dieses Playbook später nutzen kann

1. Beim Start Playbooks aus `playbooks/*.yaml` laden.
2. Eingehende Events (z. B. aus chronik oder leitstand) gegen die Trigger matchen.
3. Die beschriebenen Schritte in eine interne Aktions-Warteschlange übersetzen:
   - „Shell-Befehl lokal ausführen“ (z. B. `wgx guard ...`)
   - „GitHub-Kommentar erstellen“ (z. B. `@heimgewebe/sichter /quick`)
   - „Neues chronik-Event schreiben“
4. Ergebnisse wiederum als neue Events/Insights zurück in den Organismus speisen.

Der jetzt angelegte Prototyp ist dafür die erste, konkrete Vorlage.

---

## 5. Dialektischer Kommentar

Ein CI-Fehler auf `main` ist entweder:

- ein Ausrutscher – oder
- ein Symptom eines strukturellen Problems.

Dieses Playbook zwingt hausKI dazu, sich nicht nur zu erschrecken,
sondern aktiv und nachvollziehbar zu reagieren.

Mit anderen Worten: Der Organismus hört auf, bei Fehlern nur zu zucken,
und fängt an, reflektiert zu blinzeln.
