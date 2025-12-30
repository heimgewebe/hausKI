use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use crate::AppState;
use hauski_memory as mem;

#[derive(Debug, Deserialize)]
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

pub async fn event_handler(
    State(_state): State<AppState>,
    Json(event): Json<Event>,
) -> impl IntoResponse {
    if event.event_type == "knowledge.observatory.published.v1" {
        tracing::info!("Received observatory event, checking for decision preimages");

        // Gate check
        if let Ok(preimages) = mem::global().scan_prefix("decision.preimage:".to_string()).await {
             if !preimages.is_empty() {
                 tracing::info!("Found {} preimages, marking for re-check", preimages.len());
                 for key in preimages {
                     tracing::info!("Marking {} as needs_recheck", key);

                     // Implementation: fetch, decode, modify, set.
                     if let Ok(Some(item)) = mem::global().get(key.clone()).await {
                         // try to parse as json value
                         if let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&item.value) {
                             if let Some(obj) = json.as_object_mut() {
                                 obj.insert("needs_recheck".to_string(), serde_json::Value::Bool(true));
                                 if let Ok(new_val) = serde_json::to_vec(&obj) {
                                     let _ = mem::global().set(key, new_val, mem::TtlUpdate::Preserve, Some(item.pinned)).await;
                                 }
                             }
                         }
                     }
                 }
            } else {
                tracing::info!("No open decision preimages found.");
            }
        }
    }
    StatusCode::OK
}
