# Audio

Der Audio-Stack von HausKI kapselt alle Interaktionen mit PipeWire, ASR (whisper-rs) und TTS (piper-rs). Ziel ist eine reproduzierbare lokale Audioumgebung ohne manuelles Patchen.

## Kernprinzipien

- **Profile statt Ad-hoc-Setups:** `audio/profiles.yaml` beschreibt Geräte, Lautstärke-Presets und Routing; CLI-Kommandos aktivieren Profile deterministisch.
- **PipeWire-Facade:** Eine Rust-CLI abstrahiert `pw-cli`/`pactl` und sorgt für idempotente Umschaltungen (Studio vs. Call vs. Nachtmodus).
- **Offline-Modelle:** Whisper- und Piper-Modelle werden lokal gehalten; Konfigurationsdateien definieren Pfade und Quantisierung.
- **Budget-Kontrolle:** Audiojobs respektieren GPU/Power-Limits (über systemd-Slices und `nvidia-smi` Hooks) und melden Telemetrie nach `/metrics`.

## Workflow

1. Profile definieren/anpassen (`audio/profiles.yaml`).
2. CLI aufrufen, z. B. `cargo run -p hauski-audio -- profile switch studio` (geplant).
3. ASR/TTS-Services konsumieren die aktiven Profile und greifen auf lokal konfigurierte Modelle zu.
4. Observability: Audio-spezifische KPIs (WER, Latenz) laufen in den zentralen Budget-Guards.

## Integrationen

- **Core:** stellt `/audio/profile`-Endpoint bereit, um Profilwechsel zu triggern (Roadmap).
- **Runbooks:** Audio-spezifische Troubleshooting-Guides sollten an die Profile gekoppelt werden.
- **Security:** High-Risk-Adapter (z. B. VoIP) laufen in isolierten systemd-Slices.

## Status

Die CLI ist in Arbeit; Profil- und Budgetdefinitionen sind im Architektur-Dokument verankert. Beim Ausbau sollten Unit-Tests für Profile-Parsing sowie Integrationstests mit PipeWire-Mock ergänzt werden.
