use serde_json::json;
use tracing::info;

use policy::policy_client;
use policy::utils::events;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let decision = policy_client::decide("reminder", json!({"load": 0.3})).await?;
    events::write_event_line("policy.shadow_decide", &decision);
    let serialized = serde_json::to_string(&decision)?;
    info!(decision = %serialized, "logged shadow decision");
    println!("{}", serde_json::to_string_pretty(&decision)?);
    Ok(())
}
