use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use crate::AppState;
use hauski_memory as mem;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EventPayload {
    pub url: String,
    #[serde(default)]
    pub generated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Event {
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: EventPayload,
}

#[derive(Debug, Serialize)]
struct RecheckReason {
    #[serde(rename = "type")]
    event_type: String,
    url: String,
    generated_at: Option<String>,
}

pub async fn event_handler(
    State(_state): State<AppState>,
    Json(event): Json<Event>,
) -> impl IntoResponse {
    if event.event_type == "knowledge.observatory.published.v1" {
        tracing::info!("Received observatory event, checking for decision preimages");

        // Gate check
        if let Ok(preimages) = mem::global().scan_prefix("decision.preimage:".to_string()).await {
             if !preimages.is_empty() {
                 tracing::info!("Found {} candidate keys", preimages.len());
                 for key in preimages {
                     // Implementation: fetch, decode, filter, modify, set.
                     if let Ok(Some(item)) = mem::global().get(key.clone()).await {
                         // try to parse as json value
                         if let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&item.value) {
                             if let Some(obj) = json.as_object_mut() {

                                 // Filter: status == "open" AND needs_recheck != true
                                 let is_open = obj.get("status").and_then(|v| v.as_str()).map(|s| s == "open").unwrap_or(false);
                                 let already_flagged = obj.get("needs_recheck").and_then(|v| v.as_bool()).unwrap_or(false);

                                 if is_open && !already_flagged {
                                     tracing::info!("Marking {} as needs_recheck", key);

                                     obj.insert("needs_recheck".to_string(), serde_json::Value::Bool(true));

                                     let reason = RecheckReason {
                                         event_type: event.event_type.clone(),
                                         url: event.payload.url.clone(),
                                         generated_at: event.payload.generated_at.clone(),
                                     };

                                     if let Ok(reason_val) = serde_json::to_value(reason) {
                                         obj.insert("recheck_reason".to_string(), reason_val);
                                     }

                                     if let Ok(new_val) = serde_json::to_vec(&obj) {
                                         let _ = mem::global().set(key, new_val, mem::TtlUpdate::Preserve, Some(item.pinned)).await;
                                     }
                                 } else {
                                     tracing::debug!("Skipping {}: status is open={} or already flagged={}", key, is_open, already_flagged);
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
