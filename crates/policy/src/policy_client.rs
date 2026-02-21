use once_cell::sync::OnceCell;
use serde_json::{json, Value};
use std::time::Duration;

/// Globaler, wiederverwendeter HTTP-Client mit Timeout.
///
/// Initialisiert beim ersten Zugriff, um sicherzustellen, dass er nur einmal erstellt wird.
static HTTP_CLIENT: OnceCell<reqwest::Client> = OnceCell::new();

fn get_http_client() -> anyhow::Result<&'static reqwest::Client> {
    HTTP_CLIENT.get_or_try_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build reqwest client with timeout: {}", e))
    })
}

pub async fn decide(kind: &str, features: Value) -> anyhow::Result<Value> {
    let url = std::env::var("POLICY_URL").unwrap_or_else(|_| "http://127.0.0.1:8779".into());
    let client = get_http_client()?;
    let resp = client
        .post(format!("{url}/v1/policy/decide"))
        .json(&json!({"kind": kind, "features": features}))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    Ok(resp)
}

pub async fn feedback(
    kind: &str,
    action: &str,
    reward: f32,
    features: Option<Value>,
) -> anyhow::Result<()> {
    let url = std::env::var("POLICY_URL").unwrap_or_else(|_| "http://127.0.0.1:8779".into());
    let body = json!({"kind": kind, "action": action, "reward": reward, "features": features.unwrap_or(json!({}))});
    let client = get_http_client()?;
    client
        .post(format!("{url}/v1/policy/feedback"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
