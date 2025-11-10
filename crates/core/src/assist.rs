use axum::{http::StatusCode, Json};
use axum::{extract::State, http::Method};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, env, fs, io::Write, path::Path, time::Instant};
use utoipa::ToSchema;
use ulid::Ulid;

use crate::AppState;

#[derive(Debug, Deserialize, ToSchema)]
#[schema(title = "AssistRequest", example = json!({"question":"Wie richte ich /docs ein?","mode":"knowledge"}))]
pub struct AssistRequest {
    /// Freitext-Frage / Aufgabe.
    pub question: String,
    /// Optionaler Hint für das Routing ("code" | "knowledge").
    #[serde(default)]
    pub mode: Option<String>,
}

/// Zitat/Quelle (MVP-Struktur; später aus semantAH/Index befüllt).
#[derive(Debug, Serialize, ToSchema, Clone)]
#[schema(title = "AssistCitation", example = json!({"title":"docs/api.md","score":0.83}))]
pub struct AssistCitation {
    /// Titel/Identifier der Quelle (z. B. Dateipfad, Notiz-Titel).
    pub title: String,
    /// Score/Ähnlichkeit (0..1), falls verfügbar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(title = "AssistResponse", example = json!({
  "answer":"Router wählte knowledge. (MVP-Stub)",
  "citations":[{"title":"docs/api.md","score":0.83}],
  "trace":[{"step":"router","decision":"knowledge","reason":"heuristic"}],
  "latency_ms": 12
}))]
pub struct AssistResponse {
    /// Antworttext (MVP-Stub).
    pub answer: String,
    /// Quellenhinweise (Titel/IDs + optional Score).
    pub citations: Vec<AssistCitation>,
    /// Minimale Trace-Infos zur Entscheidung.
    pub trace: Vec<serde_json::Value>,
    /// End-to-end Latenz in Millisekunden.
    pub latency_ms: u64,
}

fn route_mode(q: &str, hint: &Option<String>) -> &'static str {
    if let Some(h) = hint {
        match h.as_str() {
            "code" => return "code",
            "knowledge" => return "knowledge",
            _ => {}
        }
    }
    // Sehr einfache Heuristik (MVP)
    let lower = q.to_ascii_lowercase();
    let looks_like_code = lower.contains("```")
        || lower.contains("fn ")
        || lower.contains("class ")
        || lower.contains("cargo ")
        || lower.contains("pip ")
        || lower.contains("error:")
        || lower.contains("traceback");
    if looks_like_code { "code" } else { "knowledge" }
}

/// Optionaler JSONL-Event-Sink (HAUSKI_EVENT_SINK=/pfad/events.jsonl)
fn event_sink_path() -> Option<String> {
    env::var("HAUSKI_EVENT_SINK").ok().filter(|s| !s.is_empty())
}

fn write_event(kind: &str, level: &str, labels: BTreeMap<&str, serde_json::Value>, data: serde_json::Value) {
    let Some(path) = event_sink_path() else { return };
    let event = serde_json::json!({
        "id": Ulid::new().to_string(),
        "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "version": "1.0.0",
        "kind": kind,
        "level": level,
        "source": "hauski-core",
        "node": hostname::get().ok().and_then(|h| h.into_string().ok()).unwrap_or_else(|| "unknown".into()),
        "labels": labels,
        "data": data
    });
    if let Err(err) = (|| -> std::io::Result<()> {
        let p = Path::new(&path);
        if let Some(dir) = p.parent() { fs::create_dir_all(dir)?; }
        let mut f = fs::OpenOptions::new().create(true).append(true).open(p)?;
        serde_json::to_writer(&mut f, &event)?;
        f.write_all(b"\n")?;
        Ok(())
    })() {
        tracing::warn!("failed to write event to sink {}: {}", path, err);
    }
}

/// Minimaler Assist-Router (MVP): wählt "code" oder "knowledge" und liefert eine Stub-Antwort.
#[utoipa::path(
    post,
    path = "/assist",
    tag = "core",
    request_body = AssistRequest,
    responses(
        (status = 200, description = "Assist response (MVP)", body = AssistResponse)
    )
)]
pub async fn assist_handler(
    State(state): State<AppState>,
    Json(req): Json<AssistRequest>,
) -> (StatusCode, Json<AssistResponse>) {
    let started = Instant::now();
    let mode = route_mode(&req.question, &req.mode);

    // TODO(Phase 2): Für "knowledge" semantAH-TopK /index/search integrieren; für "code" Tooling-Hooks.
    let answer = format!("Router wählte {}. (MVP-Stub)", mode);
    let citations = if mode == "knowledge" {
        vec![AssistCitation { title: "docs/api.md".to_string(), score: Some(0.83) }]
    } else {
        Vec::new()
    };
    let trace = vec![serde_json::json!({
        "step":"router",
        "decision":mode,
        "reason": req.mode.as_deref().unwrap_or("heuristic")
    })];

    let ms = started.elapsed().as_millis() as u64;

    // Optionale Events (JSONL) gemäß contracts/events.schema.json
    {
        let mut labels = BTreeMap::new();
        labels.insert("mode", serde_json::json!(mode));
        labels.insert("citations", serde_json::json!(citations.len()));
        write_event(
            "core.assist.request",
            "info",
            labels.clone(),
            serde_json::json!({ "question_preview": &req.question.chars().take(120).collect::<String>() })
        );
        write_event(
            "core.assist.response",
            "info",
            labels,
            serde_json::json!({
                "answer_preview": answer.chars().take(160).collect::<String>(),
                "latency_ms": ms,
                "citations": citations.iter().map(|c| serde_json::json!({
                    "title": c.title,
                    "score": c.score
                })).collect::<Vec<_>>()
            })
        );
    }

    state.record_http_observation(Method::POST, "/assist", StatusCode::OK, started);
    (StatusCode::OK, Json(AssistResponse {
        answer,
        citations,
        trace,
        latency_ms: ms,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heuristic_prefers_code_for_obvious_snippets() {
        let q = "Fehler: traceback ... ```python\nprint('hi')\n```";
        assert_eq!(route_mode(q, &None), "code");
    }

    #[test]
    fn heuristic_prefers_knowledge_for_general_questions() {
        let q = "Wie dokumentiere ich die API?";
        assert_eq!(route_mode(q, &None), "knowledge");
    }

    #[test]
    fn hint_overrides_heuristic() {
        let q = "Wie dokumentiere ich die API?";
        assert_eq!(route_mode(q, &Some("code".into())), "code");
        assert_eq!(route_mode(q, &Some("knowledge".into())), "knowledge");
    }
}
