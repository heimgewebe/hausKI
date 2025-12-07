# hausKI-Guidance im Heimgewebe-Organismus

Dieses Dokument beschreibt, wie `hausKI` im Heimgewebe-Organismus arbeiten soll:

- welche Signale es liest,
- welche Contracts es respektiert,
- wie Entscheidungen gefällt und protokolliert werden.

Es baut auf dem Organismus-Dokument im Repository `heimgewebe/metarepo` auf
(`docs/heimgewebe-organismus.md` im Metarepo).

---

## 1. Rolle von hausKI im Organismus

Kurzfassung:

- `chronik` = Gedächtnis (Events)
- `semantAH` = Sinnschicht (Insights)
- `leitstand` = Sichtbarkeit (Digest)
- `wgx` = Motorik (Kommandos)
- `heimlern` = Lernen aus Feedback
- **`hausKI` = Entscheider, der diese Schichten koordiniert**

hausKI soll:

1. Systemzustände aus den bestehenden Datenquellen lesen,
2. Entscheidungen anhand von Playbooks und Policies treffen,
3. Aktionen auslösen (z. B. WGX-Kommandos, Hinweise, Tickets),
4. jede relevante Entscheidung wieder als Event in `chronik` hinterlassen.

---

## 2. Eingänge: Was hausKI lesen darf und soll

### 2.1 Events aus `chronik`

Quelle: Files, die `event.line`-artige Einträge enthalten (siehe Contract im Metarepo).

Typische Event-Arten:

- `ci.success` / `ci.failure`
- `deploy.*`
- `metrics.snapshot`
- `hauski.decision.*` (Eigen-Entscheidungen)

Guidance:

- hausKI interpretiert nur Events, die dem `event.line`-Schema entsprechen.
- Freies Parsen von JSONL ohne Contract ist tabu.

### 2.2 Insights aus `semantAH`

Quelle: `insights.daily` (z. B. `today.json`), nach Contract im Metarepo.

hausKI nutzt:

- `topics` → Worum dreht sich der Tag?
- `questions` → Offene Denkangebote
- `deltas` → Veränderungen zu Vortagen

Guidance:

- Keine eigene Topic- oder Delta-Logik bauen, wenn semantAH sie bereits liefert.
- Einsichten immer über den `insights.daily`-Contract lesen.

### 2.3 Fleet Health (`fleet.health`)

Quelle: WGX-Metriken, exportiert als `fleet.health`-Snapshot (Contract im Metarepo).

hausKI nutzt:

- `repoCount`
- `status.ok/warn/fail`
- optionale `details[]` pro Repo

Guidance:

- Priorisierung orientiert sich an `warn/fail` und optionalen `details`.
- Rohdaten werden nicht „frei interpretiert“, sondern über den Contract gelesen.

### 2.4 Vault / menschliche Inputs

Quelle:

- Vault-Notizen,
- manueller Input,
- Leitstand-Digests in Markdown.

Guidance:

- Direktes Volltext-Parsen des Vaults ist Ausnahme, nicht Standard.
- Bevor hausKI Vault-Dateien direkt liest, wird geprüft, ob semantAH oder leitstand dafür schon eine abstrahierte Sicht bietet.

---

## 3. Entscheidungsprinzipien von hausKI

hausKI folgt vier festen Schleusen vor jeder größeren Entscheidung:

### 3.1 Contracts-first

1. Existiert ein Contract im Metarepo für diese Daten?
2. Wird er eingehalten?
3. Wenn nein: Event als „Schema-Drift“ markieren statt still akzeptieren.

Hausregel:

- Keine Entscheidungen auf Basis von Daten, die bewusst am Contract vorbei gehen.

### 3.2 Prämissencheck

Vor jeder Aktion prüft hausKI:

- Sind die Eingangsdaten vollständig genug?
- Könnte ein simpler Messfehler vorliegen (z. B. fehlender Snapshot, leere Datei)?
- Stützen mehrere Quellen dieselbe Aussage (z. B. Event + Metrics + Insights)?

Wenn grundlegende Prämissen unklar sind:

- Entscheidung verschieben oder auf „Beobachten“ herunterstufen,
- ein Event `hauski.decision.deferred` in `chronik` schreiben.

### 3.3 Alternativweg-Abwägung

hausKI zieht mindestens **zwei** Handlungswege in Betracht:

- direkten Weg (z. B. sofortiger Alarm, Issue, Eskalation),
- moderaten Weg (z. B. Beobachtung, erneute Messung, Rückfrage).

Guidance:

- Bei Unsicherheit oder schwachen Signalen eher konservativ (moderater Weg),
- bei klaren, mehrfach bestätigten Signalen kann der direkte Weg gewählt werden.

### 3.4 Risiko- und Unsicherheitsanalyse

Vor der Aktion bewertet hausKI:

- Auswirkungen (z. B. „nur CI“, „Produktivsystem“, „Datenverlust möglich“),
- Unsicherheitsgrad der Entscheidung (z. B. 0.0–1.0),
- sichtbare Nebenwirkungen (z. B. Spam-Gefahr, Rauschen).

Jede Entscheidung, die zu „starken“ Aktionen führt, sollte:

- ihren geschätzten Unsicherheitsgrad in `chronik` protokollieren,
- klar machen, auf welchen Daten sie beruht (Events, Insights, Health).

---

## 4. Ausgänge: Was hausKI tun darf

### 4.1 Events nach `chronik` schreiben

hausKI schreibt eigene Entscheidungen als `event.line`-konforme Events, z. B.:

- `hauski.decision.created_issue`
- `hauski.decision.triggered_wgx`
- `hauski.decision.deferred`

Jedes Decision-Event enthält:

- `timestamp`
- `kind`
- `repo` (falls relevant)
- `payload` mit:
  - genutzten Input-Quellen,
  - grober Begründung,
  - Unsicherheitsgrad.

### 4.2 WGX-Kommandos anstoßen

hausKI kann vorschlagen oder ausführen (je nach Modus):

- `wgx guard` / `wgx smoke` für bestimmte Repos,
- `wgx metrics` zum Erneuern von Fleet-Health.

Guidance:

- Kein direkter Shell-Zoo, sondern standardisierte WGX-Kommandos.
- Bei automatischem Ausführen immer Decision-Event in `chronik` schreiben.

### 4.3 Hinweise / Tickets / Notizen

hausKI kann:

- Hinweise in Leitstand-Digests einspeisen (z. B. durch Notizen, die später gerendert werden),
- Issues / Tickets vorschlagen,
- Notizen für `mitschreiber` generieren, die dann menschlich kuratiert werden.

Guidance:

- Automatisches Spam-Erzeugen vermeiden (z. B. nicht bei jeder kleinen Warnung ein Ticket).
- Lieber konsolidierte Hinweise mit klarer Priorität.

---

## 5. Playbook-Rahmen für hausKI

Auch wenn die konkrete Implementierung in Code erfolgt, folgt jedes Playbook grob diesem Rahmen:

1. **Trigger**
   - z. B. Event-Muster aus `chronik`, bestimmte `fleet.health`-Konstellation, neue `insights.daily`.
2. **Datenzugriff**
   - Nur über Contracts (event.line, fleet.health, insights.daily).
3. **Prämissencheck & Alternativwege**
   - Welche Annahmen stecken darin?
   - Welche Alternativaktionen sind möglich?
4. **Risiko / Unsicherheit**
   - Wie hoch ist die Unsicherheit?
   - Welche Wirkung hätte eine Fehlentscheidung?
5. **Aktion**
   - WGX-Kommando, Hinweis, Ticket, Beobachtung.
6. **Event nach chronik**
   - Entscheidung und Begründung werden protokolliert.

---

## 6. Wie dieses Dokument weiterentwickelt wird

- Neue Playbooks oder Datenquellen ergänzen hier ihre Regeln.
- Änderungen an Contracts im Metarepo werden hier sichtbar nachgezogen.
- hausKI-Implementierung verweist in Kommentaren explizit auf relevante Abschnitte dieses Dokuments.

Ziel: Entscheidungen von hausKI sind nicht „magisch“, sondern aus
Organismus-Regeln und Contracts nachvollziehbar.
