# X-Repo Event Contracts für hausKI

Dieses Dokument verankert die hausKI-Eventwelt explizit in den zentralen Heimgewebe-Contracts.

## 1. Zentrale Contracts (metarepo)

Die kanonischen Event- und Insight-Contracts liegen im Contracts-Repo (metarepo):

- `contracts/aussen.event.schema.json` – externe Roh-Events (Feeds, News, etc.)
- `contracts/event.line.schema.json` – normalisierte Event-Linien im Chronik-Backbone
- `contracts/insights.daily.schema.json` – tägliche semantische Zusammenfassungen (semantAH → leitstand → hausKI)
- `contracts/fleet.health.schema.json` – Fleet-Gesundheitszustand (wgx / Leitstand)

hausKI definiert **keine alternativen Versionen** dieser Schemas, sondern arbeitet auf Basis dieser Kanon-Contracts.

## 2. hausKI-Events im Kontext von `event.line`

Die hausKI-Contracts unter `docs/contracts/events.schema.json` beschreiben in erster Linie:

- Entscheidungs- und Kontext-Events im hausKI-Inneren,
- inkl. Referenzen auf Ereignisse aus der Chronik.

Beziehung zu `event.line`:

- `event_id` / ULID-Felder verweisen auf Einträge, die der `event.line`-Contract beschreibt.
- Felder wie `source`, `tags` oder `context` erweitern `event.line` um hausKI-spezifische Metadaten
  (z. B. welche Tools beteiligt waren, welche Policies aktiv waren).

Merksatz:

> `event.line` = „Was ist im Organismus passiert?“
> hausKI-Event = „Wie hat hausKI dieses Ereignis gesehen, bewertet und beantwortet?“

## 3. Herkunftswege (x-producers / x-consumers)

Für die Kern-Contracts gilt:

- `aussen.event`
  - Producer: `aussensensor`
  - Consumer: `chronik` (Normalisierung), ggf. direkt `semantAH`
- `event.line`
  - Producer: `chronik`
  - Consumer: `semantAH`, `hausKI`, `leitstand`
- `insights.daily`
  - Producer: `semantAH`
  - Consumer: `leitstand`, `hausKI`
- `fleet.health`
  - Producer: wgx-Jobs / Leitstand
  - Consumer: `leitstand`, `hausKI` (z. B. für Policy-Entscheidungen)

hausKI betrachtet diese Contracts als **Upstream-Wahrheit**. Eigene hausKI-Contracts dokumentieren lediglich:

- wie hausKI diese Daten konsumiert,
- welche zusätzlichen Felder hausKI intern hinzufügt,
- welche Events nicht zurück in die Chronik geschrieben werden (rein intern).

## 4. Interne vs. externe hausKI-Events

Zur Orientierung:

- **Extern anschlussfähig** (sollten sich an zentrale Contracts anlehnen):
  - Events, die in `chronik` oder `leitstand` wieder auftauchen,
  - alles, was langfristig Teil des Organismus-Gedächtnisses sein soll.

- **Rein intern**:
  - temporäre Tool-Calls,
  - Zwischenschritte im Reasoning,
  - Debug-Events, die nur für hausKI-Tracing genutzt werden.

Für externe Events sollte dieses Dokument regelmäßig angepasst werden, sobald neue Contracts im metarepo hinzukommen oder bestehende erweitert werden.

## 5. Ausblick

Langfristig ist geplant:

- die hausKI-Event-Schemas teilweise direkt auf die zentralen Contracts zu referenzieren (z. B. via `$ref`),
- zusätzliche hausKI-Contracts (z. B. für Policy-Snapshots) im zentralen Contracts-Repo zu spiegeln.

Bis dahin dient dieses Dokument als lebende Brücke zwischen:

- dem zentralen Contract-Backbone im metarepo und
- den internen Entscheidungs-Events in hausKI.
