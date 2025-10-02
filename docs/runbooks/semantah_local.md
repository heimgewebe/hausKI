# semantAH lokal betreiben

Diese Runbook-Version beschreibt den Bootstrap von semantAH auf einer HausKI-Workstation. Sie konzentriert sich auf den lokalen Pfad mit Ollama, Seeds und den WGX-Recipes.

## Voraussetzungen

- Pop!_OS oder kompatible Linux-Distribution
- Rust toolchain (stable)
- Python 3.11+
- Ollama >= 0.1.32 mit lokalem Zugriff auf `http://127.0.0.1:11434`

## 1. Ollama vorbereiten

1. Installiere Ollama gemäß [offizieller Anleitung](https://github.com/ollama/ollama).
2. Starte den Dienst:
   ```bash
   systemctl --user enable --now ollama.service
   ```
3. Lade das Embedding-Modell:
   ```bash
   ollama pull nomic-embed-text
   ```

## 2. Seeds einspielen

Das Repository liefert Beispielmaterial unter `seeds/`.

```bash
# Optional: Vault & Events in dein Arbeitsverzeichnis kopieren
mkdir -p "$HOME/.local/state/hauski/seeds"
cp -r seeds/obsidian.sample "$HOME/.local/state/hauski/seeds/obsidian"
cp seeds/semantah/events.sample.jsonl "$HOME/.local/state/hauski/seeds/events.jsonl"
```

Passe in `configs/hauski.yml` den Pfad `obsidian.vault_path` auf dein Seed-Verzeichnis an, falls du das Vault direkt nutzen möchtest.

## 3. Konfiguration validieren

```bash
cargo run -p hauski-cli -- config validate
```

Die Ausgabe bestätigt Pfade, Provider und Plug-ins. Warnungen zu fehlenden Verzeichnissen sind erwartbar, solange die Pipeline noch nicht gelaufen ist.

## 4. semantAH-Pipeline mit WGX ausführen

### Komplettlauf

```bash
wgx run semantah:all
```

- Chunkt Notizen und Canvas-Dateien
- Erstellt Embeddings (`.gewebe/embeddings.parquet`)
- Baut Graph-Dateien (`nodes.jsonl`, `edges.jsonl`)
- Aktualisiert Related-Blöcke

### Nur Related-Blöcke aktualisieren

```bash
wgx run semantah:related
```

Schreibt den Marker-Block `<!-- related:auto:start --> … <!-- related:auto:end -->` idempotent in Markdown-Dateien. Seeds enthalten eine Beispiel-Notiz zum Testen.

### Guard + Dry-Run

```bash
wgx guard semantah
```

- Führt `hauski config validate` aus
- Startet `make -C tools/semantah related CHECK=1`
- Gibt ein Diff der betroffenen Notizen aus, nimmt jedoch keine Änderungen vor

## 5. Artefakte prüfen

Nach einem Lauf findest du Artefakte unter `$HOME/.local/state/hauski/index/<namespace>/.gewebe/`.

```bash
ls -R "$HOME/.local/state/hauski/index"
```

Wichtige Dateien:

- `embeddings.parquet` — Embeddings und Chunk-Metadaten
- `nodes.jsonl`, `edges.jsonl` — Graph-Daten
- `reports/*.md` — Pipeline-Protokolle (Stub)
- Aktualisierte Obsidian-Notizen mit Related-Block

## 6. Optional: systemd-Timer aktivieren

Siehe [docs/semantah.md](../semantah.md#systemd-optional) für die Service- und Timer-Unit. Aktivieren mit:

```bash
systemctl --user enable --now hauski-semantah.timer
```

## Fehlerbehebung

- **Ollama nicht erreichbar:** Prüfe `ollama list` und Ports (`ss -tlnp | grep 11434`).
- **WGX nicht installiert:** Installiere gemäß Projektanleitung oder führe die `make`-Targets direkt aus.
- **Related-Block fehlt:** Stelle sicher, dass die Notiz den Marker enthält oder lasse `update_related.py` ihn anlegen.
