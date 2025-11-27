use heimlern_core::{Context, Decision};
use serde_json::json;

#[derive(Default, Clone)]
pub struct RemindBandit;

impl RemindBandit {
    pub fn decide(&mut self, ctx: &Context) -> Decision {
        Decision {
            action: "shadow".to_string(),
            score: 0.0,
            why: format!("shadow-mode decision for kind '{}'.", ctx.kind),
            context: Some(json!({})),
        }
    }

    pub fn feedback(&mut self, _ctx: &Context, _action: &str, _reward: f32) {}
}
