# Review-Zyklus v1.4

Der aktuelle Review-Zyklus dokumentiert, wo Codex-Läufe landen, wie die Hooks
arbeiten und welche Erweiterungen in Arbeit sind.

## Speicherpfade & Index
- Primäre Ablage: `~/.hauski/review/<repo>/` enthält Reports, Canvas-Dateien und
  Logs des jeweiligen Repositories.
- Repository-seitig empfiehlt sich ein Symlink `ln -s ~/.hauski/review/hauski
  .hauski-reports`, damit Artefakte lokal bleiben, aber nicht ins Repo gelangen.
  > ⚠️  Symlink nicht committen; die Artefakte liegen ausschließlich lokal.
- Ein globaler Index bündelt alle Runs unter `~/.hauski/review/index.json` und
  wird vom Hook laufend aktualisiert.

## Hook-Verhalten
- Das Git-Hook-Skript nutzt `flock`, um parallele Läufe zu verhindern.
- Erkennung von "Doc-only"-Changes vermeidet unnötige, teure Checks.
- Für opt-in Asynchronität lässt sich `HAUSKI_ASYNC=1` setzen; der Hook legt
  Runs dann in die Queue und kehrt schneller zum Commit zurück.

## Ausblick: `hauski-reviewd`
- Geplant ist ein Daemon, der das Review-Verzeichnis überwacht, neue Läufe
  einsammelt und in eine Warteschlange einsortiert (`watch & queue`).
- Ziel ist ein robuster Offline-Workflow ohne manuelle Trigger.

## Flags
- `chat_upstream_url`: optionaler OpenAI-kompatibler Upstream (z. B.
  llama.cpp-Server).
