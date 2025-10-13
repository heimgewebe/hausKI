use serde_json::{json, Value};

pub async fn decide(kind: &str, features: Value) -> anyhow::Result<Value> {
    let url = std::env::var("POLICY_URL").unwrap_or_else(|_| "http://127.0.0.1:8779".into());
    let resp = reqwest::Client::new()
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
    reqwest::Client::new()
        .post(format!("{url}/v1/policy/feedback"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
