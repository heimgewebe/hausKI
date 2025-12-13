use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum IntentType {
    Coding,
    Writing,
    CiTriage,
    ContractsWork,
    Unknown,
}

impl Default for IntentType {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IntentSignal {
    pub kind: String,
    pub r#ref: String,
    pub weight: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Intent {
    pub intent: IntentType,
    pub confidence: f64,
    pub signals: Vec<IntentSignal>,
    pub created_at: DateTime<Utc>,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct IntentContext {
    pub changed_paths: Vec<String>,
    pub workflow_name: Option<String>,
    pub pr_comments: Vec<String>,
}

impl Intent {
    pub fn new() -> Self {
        Self {
            intent: IntentType::Unknown,
            confidence: 0.55,
            signals: Vec::new(),
            created_at: Utc::now(),
            context: HashMap::new(),
        }
    }
}

pub struct IntentResolver {
    base_confidence: f64,
}

impl Default for IntentResolver {
    fn default() -> Self {
        Self {
            base_confidence: 0.55,
        }
    }
}

impl IntentResolver {
    pub fn resolve(&self, ctx: &IntentContext) -> Intent {
        let mut intent = Intent::new();
        intent.confidence = self.base_confidence;

        let mut counts = HashMap::new();
        counts.insert(IntentType::Coding, 0);
        counts.insert(IntentType::Writing, 0);
        counts.insert(IntentType::CiTriage, 0);
        counts.insert(IntentType::ContractsWork, 0);

        // Analyze paths
        for path_str in &ctx.changed_paths {
            let path = Path::new(path_str);
            let (t, weight) = self.classify_path(path);
            if let Some(t) = t {
                *counts.entry(t.clone()).or_insert(0) += 1;
                intent.signals.push(IntentSignal {
                    kind: "changed_path".to_string(),
                    r#ref: path_str.clone(),
                    weight,
                });
            }
        }

        // Analyze workflow
        if let Some(wf) = &ctx.workflow_name {
            intent.signals.push(IntentSignal {
                kind: "workflow".to_string(),
                r#ref: wf.clone(),
                weight: 0.7,
            });
            // Workflow usually implies CI triage or specific tasks, but here we treat it as a signal
            // If the workflow itself is being run, maybe it's CI triage?
            // The prompt says: "Wenn nur .github/workflows/ oder CI-Files -> ci_triage"
            // It also says: "GitHub Event Context (PR/push) ... optional: Commit message / changed paths / workflow name"
        }

        // Analyze comments
        for comment in &ctx.pr_comments {
             if comment.contains("/quick") || comment.contains("/review") {
                *counts.entry(IntentType::CiTriage).or_insert(0) += 5; // Strong signal
                intent.signals.push(IntentSignal {
                    kind: "issue_comment".to_string(),
                    r#ref: comment.clone(), // truncating might be good
                    weight: 1.0,
                });
             }
        }

        // Determine dominant intent
        let total_signals: i32 = counts.values().sum();
        if total_signals == 0 {
            intent.intent = IntentType::Unknown;
            // Confidence remains base (0.55) or maybe lower?
            // Prompt says: "Startwert 0.55 ... -0.20 wenn gemischt/unklar"
            // If no signals, it is unclear.
            intent.confidence -= 0.20;
        } else {
            // Simple heuristic: pick the one with most counts
            // If there's a tie, prioritize Coding > Writing > CiTriage
            let mut best_type = IntentType::Unknown;
            let mut max_count = -1;

            for (t, c) in &counts {
                if *c > max_count {
                    max_count = *c;
                    best_type = t.clone();
                } else if *c == max_count {
                     // Tie breaking
                     if *t == IntentType::Coding {
                         best_type = IntentType::Coding;
                     } else if *t == IntentType::Writing && best_type != IntentType::Coding {
                         best_type = IntentType::Writing;
                     }
                }
            }
            intent.intent = best_type;

            // Confidence adjustment
            // +0.15 if strong path signals
            // -0.20 if mixed

            // "Strong path signals": maybe if > 70% of paths agree?
            // "Mixed": if the winner has < 50% of signals?
            // The prompt says "Wenn geänderte Pfade enthalten: ... -> coding". It implies if *any*? Or dominant?
            // "Wenn docs/ ... dominant -> writing". So dominant matters.

            // Let's calculate ratio
            let ratio = if total_signals > 0 {
                max_count as f64 / total_signals as f64
            } else {
                0.0
            };

            if ratio > 0.8 {
                 intent.confidence += 0.15;
            } else if ratio < 0.6 {
                 intent.confidence -= 0.20;
            }
        }

        // Clamp confidence
        intent.confidence = intent.confidence.clamp(0.0, 1.0);

        intent
    }

    fn classify_path(&self, path: &Path) -> (Option<IntentType>, f64) {
        let path_str = path.to_string_lossy();

        if path_str.starts_with(".github/workflows/") {
            return (Some(IntentType::CiTriage), 0.9);
        }

        if path_str.starts_with("contracts/") {
            // Prompt: "Wenn nur contracts/ -> coding oder eigener Typ contracts_work (nur wenn du willst; sonst coding)"
            // I'll use coding as default unless I want to be specific. The user said "nur wenn du willst".
            // I added ContractsWork to enum, so I can use it.
            return (Some(IntentType::ContractsWork), 0.8);
        }

        if path_str.starts_with("src/") || path_str.starts_with("crates/") {
            return (Some(IntentType::Coding), 0.9);
        }

        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext {
                "rs" | "py" | "ts" | "yml" | "yaml" | "toml" | "json" => return (Some(IntentType::Coding), 0.8),
                "md" | "txt" => return (Some(IntentType::Writing), 0.8),
                _ => {}
            }
        }

        if path_str.starts_with("docs/") || path_str.to_lowercase().contains("readme") {
             return (Some(IntentType::Writing), 0.9);
        }

        (None, 0.0)
    }
}

// Helpers to gather context from environment or git
pub fn gather_context() -> Result<IntentContext> {
    let mut ctx = IntentContext::default();

    // 1. Try to get changed files from Git (local or CI)
    // In GitHub Actions, we might use specific env vars or git commands.
    // If local, `git status --porcelain` or `git diff --name-only main...`

    // For MVP, let's try `git diff --name-only HEAD` or similar if valid.
    // Or if in PR, `git diff --name-only origin/main...HEAD`.

    // Check if we are in a git repo
    if Path::new(".git").exists() {
        // 1. Uncommitted changes (staged and unstaged) relative to HEAD
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD"])
            .output();

        if let Ok(output) = output {
             let stdout = String::from_utf8_lossy(&output.stdout);
             for line in stdout.lines() {
                 let line = line.trim();
                 if !line.is_empty() && !ctx.changed_paths.contains(&line.to_string()) {
                     ctx.changed_paths.push(line.to_string());
                 }
             }
        }

        // 2. Committed changes relative to main (for CI/PR context)
        // We try origin/main, failing that, just main.
        let output_branch = Command::new("git")
            .args(["diff", "--name-only", "origin/main...HEAD"])
            .output();

        // If origin/main failed, maybe we are detached or origin is not fetched, try just checking HEAD^ if simple commit?
        // Or just fail gracefully.

        if let Ok(output) = output_branch {
             let stdout = String::from_utf8_lossy(&output.stdout);
             for line in stdout.lines() {
                 let line = line.trim();
                 if !line.is_empty() && !ctx.changed_paths.contains(&line.to_string()) {
                     ctx.changed_paths.push(line.to_string());
                 }
             }
        }
    }

    // 2. GitHub Context
    if let Ok(workflow) = std::env::var("GITHUB_WORKFLOW") {
        ctx.workflow_name = Some(workflow);
    }

    // 3. Issue Comments (from event.json if available)
    if let Ok(event_path) = std::env::var("GITHUB_EVENT_PATH") {
        if let Ok(content) = std::fs::read_to_string(&event_path) {
             if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                 if let Some(comment) = json.get("comment").and_then(|c| c.get("body")).and_then(|b| b.as_str()) {
                     ctx.pr_comments.push(comment.to_string());
                 }
             }
        }
    }

    Ok(ctx)
}

#[cfg(test)]
#[path = "intent_tests.rs"]
mod tests;
