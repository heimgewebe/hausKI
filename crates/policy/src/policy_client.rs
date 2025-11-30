//! HTTP client for the policy service.
//!
//! This module provides functions to interact with the policy service API
//! for making decisions and providing feedback.

use serde_json::{json, Value};

/// Requests a policy decision from the policy service.
///
/// Makes a POST request to `/v1/policy/decide` with the given kind and features.
///
/// # Arguments
///
/// * `kind` - The type of decision being requested
/// * `features` - Feature vector as a JSON value
///
/// # Returns
///
/// A JSON value containing the decision response from the policy service.
///
/// # Errors
///
/// Returns an error if the HTTP request fails or the response cannot be parsed.
///
/// # Environment Variables
///
/// * `POLICY_URL` - Base URL of the policy service (default: `http://127.0.0.1:8779`)
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

/// Sends feedback about a policy decision to the policy service.
///
/// Makes a POST request to `/v1/policy/feedback` with the given parameters.
///
/// # Arguments
///
/// * `kind` - The type of decision that was made
/// * `action` - The action that was taken
/// * `reward` - Reward signal indicating the quality of the decision
/// * `features` - Optional feature vector that was used for the decision
///
/// # Errors
///
/// Returns an error if the HTTP request fails.
///
/// # Environment Variables
///
/// * `POLICY_URL` - Base URL of the policy service (default: `http://127.0.0.1:8779`)
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
