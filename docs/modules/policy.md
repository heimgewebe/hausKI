# Modul: Policy & Limits

**Rolle:** Durchsetzung von Laufzeit- und Ressourcenrichtlinien
**Crate:** `hauski-policy`

---

## Überblick

Das Policy-Modul definiert und überwacht **System-Budgets** für Performance,
Thermik, LLM-Latenz und weitere Ressourcenparameter.
Es stellt eine gemeinsame Policy-Engine für hausKI-Komponenten bereit.

---

## Hauptaufgaben

- Validieren von Konfigurationen gegen Policies
- Laufzeitprüfung (Budget-Überwachung)
- Bereitstellen strukturierter Policy-Events für `leitstand`

---

## Beispielkonfiguration (`policies/limits.yaml`)

```yaml
latency:
  llm_p95_ms: 400
  index_p95_ms: 60
thermal:
  gpu_max_c: 80
  power_watt_dgpu: 220
asr:
  wer_max: 0.10
```

---

## Schnittstellen

- `PolicySnapshot` – serialisierbarer Zustand
- `BudgetGuard` – automatisierte Prüfmechanik
- Export via `/policies` API

---

## ToDo
- [ ] Event-Schema in `contracts/` dokumentieren
- [ ] CLI-Integration `hauski-cli policy check` ergänzen

**Letzte Aktualisierung:** 2025-10-23
