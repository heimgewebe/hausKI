//! Contextual bandit implementation for policy decisions.
//!
//! This module implements a simple epsilon-greedy contextual bandit algorithm
//! for making and learning from policy decisions over time.

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

/// A contextual bandit that uses epsilon-greedy exploration.
///
/// The bandit maintains statistics for each action and chooses actions
/// based on their historical performance, with occasional random exploration.
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

/// Context information for making a policy decision.
#[derive(Debug, Clone)]
pub struct DecisionContext {
    /// The type of decision being made.
    pub kind: String,
    /// Feature vector for the decision.
    pub features: Value,
}

/// The outcome of a policy decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOutcome {
    /// The action that was chosen.
    pub action: String,
    /// Additional parameters for the action.
    #[serde(default)]
    pub parameters: Value,
}

impl RemindBandit {
    /// Loads bandit state from a JSON snapshot.
    ///
    /// If the snapshot cannot be deserialized, the bandit state remains unchanged.
    pub fn load(&mut self, snapshot: Value) {
        if let Ok(loaded) = serde_json::from_value::<Self>(snapshot) {
            *self = loaded;
        }
    }

    /// Creates a JSON snapshot of the current bandit state.
    ///
    /// Returns an empty object if serialization fails.
    pub fn snapshot(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| json!({}))
    }

    /// Makes a decision based on the given context.
    ///
    /// Chooses the action with the highest average reward. If no statistics
    /// are available, falls back to the first action in the action list.
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

    /// Provides feedback about a decision.
    ///
    /// Updates the statistics for the given action with the observed reward.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context in which the decision was made (currently unused)
    /// * `action` - The action that was taken
    /// * `reward` - The reward observed for this action
    pub fn feedback(&mut self, ctx: &DecisionContext, action: &str, reward: f32) {
        let _ = ctx;
        let entry = self.stats.entry(action.to_string()).or_default();
        entry.plays = entry.plays.saturating_add(1);
        entry.reward += reward as f64;
    }

    fn best_action(&self) -> Option<String> {
        self.actions.iter().cloned().max_by(|a, b| {
            let left = self.stats.get(a).map(ArmStats::average).unwrap_or_default();
            let right = self.stats.get(b).map(ArmStats::average).unwrap_or_default();
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
