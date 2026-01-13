# HausKI System Signals Contract

**Contract:** `hauski.system.signals.v1`  
**Schema:** [`system.signals.v1.schema.json`](./system.signals.v1.schema.json)  
**Owner:** `hauski-core`

## Zweck

Dieses Contract definiert das kanonische Format für Systemsignale, die vom HausKI Core Service bereitgestellt werden. Diese Metriken dienen als Input für Meta-Cognitive-Monitoring und Selbstmodellierung (Heimgeist).

## Struktur

- **cpu_load** (number, 0-100): Globale CPU-Auslastung in Prozent, geglättet via Exponential Moving Average (EMA, alpha=0.1)
- **memory_pressure** (number, 0-100): Speicherdruck in Prozent, geglättet via EMA (alpha=0.1)
- **gpu_available** (boolean): Heuristische Prüfung, ob eine NVIDIA GPU verfügbar ist (einmalig beim Start via `nvidia-smi -L`)

## Implementierungsdetails

- **Update-Intervall:** 2 Sekunden
- **Glättung:** EMA mit alpha=0.1 (10% aktueller Wert, 90% vorheriger EMA-Wert)
- **GPU-Erkennung:** Best-effort via `nvidia-smi`, silent fallback auf `false` bei Nicht-NVIDIA-Systemen
- **Plattformtoleranz:** Funktioniert auf Pop!_OS (NVIDIA-RTX), CI, WSL, Codespaces, Termux

## Endpunkt

- **GET** `/system/signals`
- **Response:** HTTP 200, Body: JSON gemäß Schema
- **Fehler:** HTTP 500 bei internen Fehlern (Lock-Vergiftung o. ä.)

## Versionierung

- **v1:** Initiale Version (Januar 2026)
- Änderungen an Feldern oder Semantik erfordern neue Versionsnummer (v2, v3, ...)

## Beispiel

```json
{
  "cpu_load": 23.5,
  "memory_pressure": 67.2,
  "gpu_available": true
}
```

## Validierung

Implementierungen müssen sicherstellen:
- Alle drei Felder sind vorhanden (`required`)
- `cpu_load` und `memory_pressure` liegen in [0.0, 100.0]
- `gpu_available` ist ein Boolean
- Keine zusätzlichen Felder (`additionalProperties: false`)

## Siehe auch

- [events.schema.json](../events.schema.json) – Allgemeines Event-Format
- ADR-0003 – Fehlerbehandlung und Robustheit
