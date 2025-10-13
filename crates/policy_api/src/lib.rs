pub mod utils {
    pub mod events;
}

#[cfg(feature = "heimlern")]
pub mod heimlern {
    pub use heimlern_bandits::RemindBandit;
    pub use heimlern_core::{Context, Decision};
}

#[cfg(not(feature = "heimlern"))]
pub mod heimlern {
    use serde_json::{json, Value};

    #[derive(Clone, Debug)]
    pub struct Context {
        pub kind: String,
        pub features: Value,
    }

    #[derive(Clone, Debug)]
    pub struct Decision {
        pub action: String,
        pub score: f32,
        pub why: String,
        pub context: Option<Value>,
    }

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
}
