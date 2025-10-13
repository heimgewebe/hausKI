use heimlern_core::{Context, Decision, Policy};

#[derive(Default, Clone)]
pub struct RemindBandit;

impl Policy for RemindBandit {
    fn decide(&mut self, ctx: &Context) -> Decision {
        Decision {
            action: "control".to_string(),
            score: 0.0,
            why: format!("shadow-mode decision for kind '{}'.", ctx.kind),
            context: Some(serde_json::json!({})),
        }
    }

    fn feedback(&mut self, _ctx: &Context, _action: &str, _reward: f32) {}
}

impl RemindBandit {
    pub fn decide(&mut self, ctx: &Context) -> Decision {
        <Self as Policy>::decide(self, ctx)
    }

    pub fn feedback(&mut self, ctx: &Context, action: &str, reward: f32) {
        <Self as Policy>::feedback(self, ctx, action, reward)
    }
}
