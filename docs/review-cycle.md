# Review-Zyklus v1.4
- Reports liegen unter `~/.hauski/review/<repo>/`, im Repo nur Symlink `.hauski-reports`.
- Globaler Index: `~/.hauski/review/index.json`.
- Hook arbeitet mit `flock`, erkennt "Doc-only" Changes.
- Asynchron optional via `HAUSKI_ASYNC=1`.
- Perspektive: Daemon `hauski-reviewd` (watch & queue).

## Flags
- `chat_upstream_url`: optionaler OpenAI-kompatibler Upstream (z. B. llama.cpp Server).
