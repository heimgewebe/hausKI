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

                                    let reason = build_recheck_reason(
                                        &event.event_type,
                                        &event.payload.url,
                                        event.payload.generated_at.as_deref(),
                                        event.payload.sha.as_deref(),
                                        event.payload.schema_ref.as_deref(),
                                    );

                                    obj.insert("recheck_reason".to_string(), reason);

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

/// Constructs a recheck reason JSON object.
///
/// Keys: type, url, generated_at, (optional) sha, (optional) schema_ref.
fn build_recheck_reason(
    event_type: &str,
    url: &str,
    generated_at: Option<&str>,
    sha_input: Option<&str>,
    schema_ref_input: Option<&str>,
) -> serde_json::Value {
    let mut reason = serde_json::json!({
        "type": event_type,
        "url": url,
        "generated_at": generated_at,
    });

    let obj = reason
        .as_object_mut()
        .expect("json macro should return an object");

    // SHA-Check ist syntax-only (len=64 hex), keine Inhaltsvalidierung.
    if let Some(s) = sha_input {
        let raw_hex = s.strip_prefix("sha256:").unwrap_or(s);
        if raw_hex.len() == 64 && raw_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            obj.insert(
                "sha".to_string(),
                serde_json::Value::String(format!("sha256:{}", raw_hex.to_ascii_lowercase())),
            );
        } else {
            tracing::warn!(
                "Invalid SHA format (syntax-only check failed), dropping: {}",
                s
            );
        }
    }

    if let Some(s) = schema_ref_input {
        if should_store_schema_ref(s) {
            obj.insert("schema_ref".to_string(), serde_json::Value::String(s.to_string()));
        }
    }

    reason
}

/// Validates if a schema_ref is allowed to be stored.
/// Policy:
/// - Must be a valid URL
/// - Scheme must be "https"
/// - Host must be "schemas.heimgewebe.org" (Trust Anchor)
/// - Multi-env (e.g. staging) allowed only via future policy change.
fn should_store_schema_ref(s: &str) -> bool {
    if let Ok(u) = url::Url::parse(s) {
        if u.scheme() == "https" && u.host_str() == Some("schemas.heimgewebe.org") {
            return true;
        }
        tracing::warn!(
            "schema_ref not allowed (must be https://schemas.heimgewebe.org): {}, dropping",
            s
        );
    } else {
        tracing::warn!("Invalid schema_ref URL, dropping: {}", s);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_recheck_reason_minimal() {
        let reason = build_recheck_reason("test.event", "https://example.com", None, None, None);
        assert_eq!(reason["type"], "test.event");
        assert_eq!(reason["url"], "https://example.com");
        assert!(reason.get("generated_at").unwrap().is_null());
        assert!(reason.get("sha").is_none());
        assert!(reason.get("schema_ref").is_none());
    }

    #[test]
    fn test_build_recheck_reason_full_valid() {
        let sha = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let schema = "https://schemas.heimgewebe.org/contracts/events/knowledge.observatory.published.v1.schema.json";
        let reason = build_recheck_reason(
            "test.event",
            "https://example.com",
            Some("2023-10-27T10:00:00Z"),
            Some(sha),
            Some(schema),
        );

        assert_eq!(reason["sha"], format!("sha256:{}", sha));
        assert_eq!(reason["schema_ref"], schema);
        assert_eq!(reason["generated_at"], "2023-10-27T10:00:00Z");
    }

    #[test]
    fn test_build_recheck_reason_invalid_inputs_dropped() {
        let reason = build_recheck_reason(
            "test.event",
            "https://example.com",
            None,
            Some("too-short"),
            Some("http://wrong-scheme.org"),
        );

        assert!(reason.get("sha").is_none());
        assert!(reason.get("schema_ref").is_none());
    }
}
