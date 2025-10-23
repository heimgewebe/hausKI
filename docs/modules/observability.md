# Modul: Observability

**Rolle:** Überwachung, Logging, Metriken, Tracing  
**Verortung:** Querschnittsmodul (Core-Integration)

---

## Überblick

Das Observability-Modul bündelt die Mess- und Diagnosepfade:

- **Logs:** strukturiert via `tracing` + `tracing-subscriber`
- **Metriken:** Prometheus-Exporter unter `/metrics`
- **Tracing:** span-basiert mit korrelierbaren Request-IDs
- **Budgets:** definierte SLOs in `policies/limits.yaml`

---

## Aufbau

| Komponente | Zweck |
|-------------|-------|
| **Logger** | Ausgabe strukturierter Events (JSON/ANSI) |
| **Exporter** | Prometheus-kompatibler HTTP-Endpoint |
| **AppMetrics** | fasst Latenz, Fehler und Ressourcennutzung zusammen |

---

## Beispiel-Budgets

```yaml
latency:
  llm_p95_ms: 400
  index_p95_ms: 60
thermal:
  gpu_max_c: 80
  power_watt_dgpu: 220
```

---

## Troubleshooting

- Prüfen: `curl -s localhost:8080/metrics | grep hauski`
- Health-Check: `curl -s localhost:8080/health`
- Log-Level erhöhen: `RUST_LOG=hauski=trace`

---

## Geplante Erweiterungen
- [ ] OpenTelemetry-Exporter
- [ ] Integration in `hauski-reviewd`

---

**Letzte Aktualisierung:** 2025-10-23
