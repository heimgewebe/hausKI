use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ArmStats {
    plays: u64,
    reward: f64,
}

impl ArmStats {
    fn average(&self) -> f64 {
        if self.plays == 0 {
            0.0
        } else {
            self.reward / self.plays as f64
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemindBandit {
    actions: Vec<String>,
    stats: HashMap<String, ArmStats>,
    #[serde(default = "default_epsilon")]
    epsilon: f64,
}

fn default_epsilon() -> f64 {
    0.1
}

impl Default for RemindBandit {
    fn default() -> Self {
        let actions = vec!["notify".to_string(), "snooze".to_string()];
        let stats = actions
            .iter()
            .map(|action| (action.clone(), ArmStats::default()))
            .collect();
        Self {
            actions,
            stats,
            epsilon: default_epsilon(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecisionContext {
    pub kind: String,
    pub features: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOutcome {
    pub action: String,
    #[serde(default)]
    pub parameters: Value,
}

impl RemindBandit {
    pub fn load(&mut self, snapshot: Value) {
        if let Ok(loaded) = serde_json::from_value::<Self>(snapshot) {
            *self = loaded;
        }
    }

    pub fn snapshot(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| json!({}))
    }

    pub fn decide(&mut self, ctx: &DecisionContext) -> DecisionOutcome {
        let _ = ctx;
        let action = self.best_action().unwrap_or_else(|| {
            self.actions
                .first()
                .cloned()
                .unwrap_or_else(|| "notify".into())
        });

        DecisionOutcome {
            action,
            parameters: json!({}),
        }
    }

    pub fn feedback(&mut self, ctx: &DecisionContext, action: &str, reward: f32) {
        let _ = ctx;
        let entry = self.stats.entry(action.to_string()).or_default();
        entry.plays = entry.plays.saturating_add(1);
        entry.reward += reward as f64;
    }

    fn best_action(&self) -> Option<String> {
        self.actions.iter().cloned().max_by(|a, b| {
            let left = self.stats.get(a).map(|s| s.average()).unwrap_or_default();
            let right = self.stats.get(b).map(|s| s.average()).unwrap_or_default();
            left.partial_cmp(&right).unwrap_or(Ordering::Equal)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_action_with_highest_average() {
        let mut bandit = RemindBandit::default();
        bandit.feedback(
            &DecisionContext {
                kind: "reminder".into(),
                features: json!({}),
            },
            "notify",
            1.0,
        );
        bandit.feedback(
            &DecisionContext {
                kind: "reminder".into(),
                features: json!({}),
            },
            "snooze",
            0.2,
        );

        let decision = bandit.decide(&DecisionContext {
            kind: "reminder".into(),
            features: json!({}),
        });
        assert_eq!(decision.action, "notify");
    }
}
