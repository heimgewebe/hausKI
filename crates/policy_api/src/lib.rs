//! Policy API crate providing types and utilities for policy decision-making.
//!
//! This crate provides the core types and utilities used for policy decisions
//! in the HausKI system, including event handling and contextual bandit implementations.

/// Utility modules for policy API functionality.
pub mod utils {
    /// Event handling utilities.
    pub mod events;
}

/// Heimlern contextual bandit implementation.
///
/// When the `heimlern` feature is enabled, this module provides the actual
/// RemindBandit implementation from the heimlern-bandits crate.
#[cfg(feature = "heimlern")]
pub mod heimlern {
    pub use heimlern_bandits::RemindBandit;
    pub use heimlern_core::{Context, Decision};
}

/// Shadow implementation when the `heimlern` feature is disabled.
///
/// This module provides stub implementations for testing and development
/// when the full heimlern implementation is not available.
#[cfg(not(feature = "heimlern"))]
pub mod heimlern {
    use serde_json::{json, Value};

    /// Decision context containing kind and features.
    #[derive(Clone, Debug)]
    pub struct Context {
        /// The kind of decision being made.
        pub kind: String,
        /// Feature vector for the decision.
        pub features: Value,
    }

    /// A decision made by the policy engine.
    #[derive(Clone, Debug)]
    pub struct Decision {
        /// The action to take.
        pub action: String,
        /// Confidence score for this decision.
        pub score: f32,
        /// Explanation of why this decision was made.
        pub why: String,
        /// Additional context information.
        pub context: Option<Value>,
    }

    /// Shadow implementation of RemindBandit for testing.
    #[derive(Default, Clone)]
    pub struct RemindBandit;

    impl RemindBandit {
        /// Makes a decision based on the given context.
        ///
        /// In shadow mode, always returns a fixed "shadow" action.
        pub fn decide(&mut self, ctx: &Context) -> Decision {
            Decision {
                action: "shadow".to_string(),
                score: 0.0,
                why: format!("shadow-mode decision for kind '{}'.", ctx.kind),
                context: Some(json!({})),
            }
        }

        /// Provides feedback for a decision (no-op in shadow mode).
        pub fn feedback(&mut self, _ctx: &Context, _action: &str, _reward: f32) {}
    }
}
