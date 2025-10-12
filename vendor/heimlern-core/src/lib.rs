use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Context {
    pub kind: String,
    pub features: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision {
    pub action: String,
    pub score: f32,
    pub why: String,
    pub context: Option<Value>,
}

pub trait Policy {
    fn decide(&mut self, ctx: &Context) -> Decision;
    fn feedback(&mut self, ctx: &Context, action: &str, reward: f32);
}
