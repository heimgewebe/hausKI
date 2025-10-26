# Lokaler Chat mit HausKI (llama.cpp Upstream)

Ziel: HausKI lokal „sprechfähig“ machen, indem `/v1/chat` an einen OpenAI-kompatiblen **llama.cpp**-Server proxied wird.

**Neu:** tmux-Start, `.env`-Variablen, Logs mit einfacher Rotation, Stop-Script.

## Voraussetzungen

- Ein lokales GGUF-Modell (z. B. in `~/models/your-model.gguf`)
- Optional: ein `llama.cpp`-Server-Binary (oder via `just llama-server`, falls vorhanden)
- HausKI Core lauffähig (`cargo build` bzw. `just run-core`)
- **System-Tools:**
  - `tmux` (parallele Fenster für Upstream & Core)
  - `lsof` (für Prozess-Stop via `stop-all.sh`)
  - Optional: `just` (Task-Runner für Kurzaufrufe)

## Schnellstart: Ein-Kommando-Variante

```bash
scripts/start-all.sh \
  --model "$HOME/models/your-model.gguf" \
  --port 8081
  # Optional:
  # --tmux                 # startet Upstream & Core in einer tmux-Session "hauski"
  # --no-upstream          # startet nur Core (wenn externer Upstream schon läuft)
  # --upstream-url URL     # überschreibt Host/Port; externe URLs überspringen den lokalen Start automatisch
```

Das Skript:
1. startet (falls verfügbar) den `llama.cpp`-Server auf Port 8081,
2. schreibt eine Flags-Datei mit `chat_upstream_url=http://127.0.0.1:8081` unter `~/.config/hauski/hauski-flags.yaml` und setzt `HAUSKI_FLAGS` darauf,
   > Wenn `HAUSKI_FLAGS` bereits gesetzt ist (z. B. auf eine eigene YAML), bleibt der bestehende Pfad unverändert.
3. startet anschließend den HausKI-Core.

### Start über `.env`
Lege im Repo-Root eine `.env` (oder `configs/.env`) an (siehe `.env.example`):

```
MODEL="$HOME/models/your-model.gguf"
PORT=8081
# Optional:
# UPSTREAM_URL="http://127.0.0.1:8081"
# USE_TMUX=1
# NO_UPSTREAM=1
```

Dann reicht: `scripts/start-all.sh --tmux`

> Wenn kein passendes `llama`-Binary gefunden wird, gibt das Skript einen klaren Hinweis mit Beispielbefehlen aus.

## Manuell (Einzelschritte)

1. **llama.cpp-Server** (Beispiel):
   ```bash
   llama-server \
     --port 8081 \
     --model "$HOME/models/your-model.gguf" \
     --ctx-size 4096 \
     --batch-size 512
   ```

2. **HausKI-Core**:
   - Entweder per `just run-core`
   - Oder direkt: `cargo run -p hauski-cli -- serve`

3. **Testaufruf**:
   ```bash
   curl -s -X POST http://127.0.0.1:8080/v1/chat \
     -H 'Content-Type: application/json' \
     -d '{"messages":[{"role":"user","content":"Hallo HausKI – hörst du mich?"}]}'
   ```

   - **501 Not Implemented**: Upstream nicht erreichbar → prüfe, ob `llama.cpp` auf Port 8081 läuft.
   - **200 OK + Antwort**: Alles gut.

## Konfiguration

- Der Upstream lässt sich über `configs/flags.yaml` setzen:
  ```yaml
  chat_upstream_url: "http://127.0.0.1:8081"
  ```
  (Das Startskript legt dafür die Datei `~/.config/hauski/hauski-flags.yaml` an und setzt `HAUSKI_FLAGS` auf diesen Pfad.)

## Tipps

- **Modelle**: Wähle ein kleines bis mittleres GGUF-Modell für den Anfang (z. B. 7-13B-Klasse).
- **Stabilität**: Starte `llama.cpp` zuerst, warte kurz, dann den Core. Mit `--no-upstream` (oder einer externen `--upstream-url`) überspringt das Skript den lokalen Start (und das Port-Waiting).
- **Fehlersuche**: Logs prüfen (Terminal-Ausgabe von `llama-server` und HausKI-Core).

## Logs & Stoppen

- Logs landen standardmäßig unter `~/.local/state/hauski/logs/`:
  - `upstream.log`, `core.log` (Rotation: `.1`, `.2` bei >5 MB)
- Prozesse sauber beenden:
  ```bash
  scripts/stop-all.sh
  ```
  - Bei `--tmux`: tmux-Session `hauski` wird beendet.
  - Ohne tmux: laufende PIDs (falls bekannt) werden beendet, ansonsten Ports geprüft (Port aus Argument **oder** `.env` → `PORT`).

---

**Essenz:** HausKI redet lokal, wenn ein OpenAI-kompatibler Upstream (llama.cpp) auf `127.0.0.1:8081` bereitsteht – Startskript erledigt die Orchestrierung.

**∆-Radar:** Dieser Runbook-Eintrag führt eine klarere Start-Choreografie ein (Upstream→Core) und reduziert ad-hoc-Kommandos.
