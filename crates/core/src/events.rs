use crate::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use hauski_memory as mem;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EventPayload {
    pub url: String,
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub schema_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Event {
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: EventPayload,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecheckReason {
    #[serde(rename = "type")]
    event_type: String,
    url: String,
    generated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_ref: Option<String>,
}

pub async fn event_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(event): Json<Event>,
) -> impl IntoResponse {
    // 1. Authorization Gate
    if let Some(token) = &state.flags().events_token {
        // Token is configured, must match
        let auth_header = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|val| val.to_str().ok());

        let valid = match auth_header {
            Some(h) if h.starts_with("Bearer ") => {
                let provided = h.trim_start_matches("Bearer ");
                provided == token
            }
            _ => false,
        };

        if !valid {
            tracing::warn!("Unauthorized access attempt to /events");
            return StatusCode::UNAUTHORIZED;
        }
    } else {
        // Token is NOT configured -> Endpoint Disabled (403 Forbidden)
        tracing::warn!("/events endpoint is disabled (HAUSKI_EVENTS_TOKEN not set)");
        return StatusCode::FORBIDDEN;
    }

    // 2. HTTPS enforcement for URL (SSRF prevention)
    if !event.payload.url.starts_with("https://") {
        tracing::warn!("Rejected event with non-https URL: {}", event.payload.url);
        return StatusCode::BAD_REQUEST;
    }

    if event.event_type == "knowledge.observatory.published.v1" {
        tracing::info!("Received observatory event, checking for decision preimages");

        // Gate check
        if let Ok(preimages) = mem::global()
            .scan_prefix("decision.preimage:".to_string())
            .await
        {
            if !preimages.is_empty() {
                tracing::info!("Found {} candidate keys", preimages.len());
                for key in preimages {
                    // Implementation: fetch, decode, filter, modify, set.
                    if let Ok(Some(item)) = mem::global().get(key.clone()).await {
                        // try to parse as json value
                        if let Ok(mut json) =
                            serde_json::from_slice::<serde_json::Value>(&item.value)
                        {
                            if let Some(obj) = json.as_object_mut() {
                                // Filter: status == "open" AND needs_recheck != true
                                let is_open = obj
                                    .get("status")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s == "open")
                                    .unwrap_or(false);
                                let already_flagged = obj
                                    .get("needs_recheck")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);

                                if is_open && !already_flagged {
                                    tracing::info!("Marking {} as needs_recheck", key);

                                    obj.insert(
                                        "needs_recheck".to_string(),
                                        serde_json::Value::Bool(true),
                                    );

                                    let sha = event.payload.sha.as_ref().and_then(|s| {
                                        // SHA-Check ist syntax-only (len=64 hex), keine Inhaltsvalidierung.
                                        // Allow input with or without 'sha256:' prefix
                                        let raw_hex = s.strip_prefix("sha256:").unwrap_or(s);
                                        if raw_hex.len() == 64
                                            && raw_hex.chars().all(|c| c.is_ascii_hexdigit())
                                        {
                                            // Always store canonical format: sha256:<lowercase-hex>
                                            Some(format!("sha256:{}", raw_hex.to_ascii_lowercase()))
                                        } else {
                                            tracing::warn!(
                                                "Invalid SHA format (syntax-only check failed), dropping: {}",
                                                s
                                            );
                                            None
                                        }
                                    });

                                    let schema_ref =
                                        event.payload.schema_ref.as_deref().and_then(|s| {
                                            // schema_ref ist bewusst Trust Anchor auf https://schemas.heimgewebe.org/... (Host+Scheme),
                                            // damit keine fremden Schemas in den Zustand einsickern.
                                            // Hinweis: Spätere Multi-Env-Hosts (z.B. staging) nur via bewusster Policy-Änderung erlaubt.
                                            if let Ok(u) = url::Url::parse(s) {
                                                if u.scheme() == "https"
                                                    && u.host_str()
                                                        == Some("schemas.heimgewebe.org")
                                                {
                                                    return Some(s.to_string());
                                                }
                                                tracing::warn!(
                                                "schema_ref not allowed (must be https://schemas.heimgewebe.org): {}, dropping",
                                                s
                                            );
                                            } else {
                                                tracing::warn!(
                                                    "Invalid schema_ref URL, dropping: {}",
                                                    s
                                                );
                                            }
                                            None
                                        });

                                    let reason = RecheckReason {
                                        event_type: event.event_type.clone(),
                                        url: event.payload.url.clone(),
                                        generated_at: event.payload.generated_at.clone(),
                                        sha,
                                        schema_ref,
                                    };

                                    if let Ok(reason_val) = serde_json::to_value(reason) {
                                        obj.insert("recheck_reason".to_string(), reason_val);
                                    }

                                    if let Ok(new_val) = serde_json::to_vec(&obj) {
                                        let _ = mem::global()
                                            .set(
                                                key,
                                                new_val,
                                                mem::TtlUpdate::Preserve,
                                                Some(item.pinned),
                                            )
                                            .await;
                                    }
                                } else {
                                    tracing::debug!(
                                        "Skipping {}: status is open={} or already flagged={}",
                                        key,
                                        is_open,
                                        already_flagged
                                    );
                                }
                            }
                        }
                    }
                }
            } else {
                tracing::info!("No decision preimages found.");
            }
        }
    }
    StatusCode::OK
}
