# Runbook: Upgrade-Guide

**Ziel:** Sicheres Aktualisieren von hausKI, seinen Modulen und Modellen unter Beibehaltung der Offline-Fähigkeit.

---

## 1. Vorbereitung

- Alle laufenden Dienste beenden (`systemctl --user stop hauski*`)  
- Arbeitsverzeichnis sauber halten (`git status` → clean)  
- Optional: Review-Snapshot sichern (`~/.hauski/review/<repo>/`)  

---

## 2. Toolchains prüfen

```bash
cat toolchain.versions.yml
uv --version
rustc --version
```

Wenn Versionen abweichen:
```bash
uv self update
rustup self update
rustup update stable
```

---

## 3. Vendor-Snapshot erneuern

```bash
rm -rf vendor/
cargo vendor --locked
git add vendor/
git commit -m "refresh vendor snapshot"
```

Damit bleiben Offline-Builds reproduzierbar.

---

## 4. Models & Seeds aktualisieren

- **Ollama-Modelle:** `ollama pull all-MiniLM-L6-v2`  
- **Whisper/Piper:** Download über `just ai-sync`  
- **semantAH Seeds:** `uv run scripts/build_index.py`

---

## 5. Upgrade-Checkliste

| Prüfschritt | Soll-Zustand |
|--------------|--------------|
| `cargo test --workspace` | ✅ alle grün |
| `just lint` | ✅ keine Warnungen |
| `/metrics` erreichbar | ✅ 200 OK |
| `hauski review` erzeugt Report | ✅ Index aktualisiert |

---

**Letzte Aktualisierung:** 2025-10-23

---

## Verknüpfte Dokumente

- [ADR-0001 Toolchain-Strategie](../adrs/ADR-0001-toolchain-strategy.md) — legt fest, wie Versionen zentral gepflegt werden  
- [Audit-Bericht](../audit-hauski.md) — referenziert die Toolchain-Synchronisierung und CI-Empfehlungen  
- [Runbook: Incident-Response](incident-response.md) — beschreibt, wie bei fehlerhaften Upgrades oder Policy-Verletzungen vorzugehen ist  

Diese Seite ergänzt den strategischen Upgrade-Pfad und verbindet ADRs mit operativer Praxis.
