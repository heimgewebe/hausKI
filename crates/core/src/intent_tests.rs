use crate::intent::{
    gather_context_with_provider, ContextProvider, IntentContext, IntentResolver, IntentType,
};
use anyhow::{anyhow, Result};
use std::collections::HashMap;

// Re-implement MockContextProvider to handle the Git Output cloning issue better
struct MockContextProviderRefined {
    git_outputs: HashMap<String, Result<String, String>>, // Store error as String
    env_vars: HashMap<String, String>,
    files: HashMap<String, String>,
    existing_paths: Vec<String>,
}

impl MockContextProviderRefined {
    fn new() -> Self {
        Self {
            git_outputs: HashMap::new(),
            env_vars: HashMap::new(),
            files: HashMap::new(),
            existing_paths: Vec::new(),
        }
    }

    fn with_git_output(mut self, args: &[&str], output: Result<String, String>) -> Self {
        let key = args.join(" ");
        self.git_outputs.insert(key, output);
        self
    }

    fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env_vars.insert(key.to_string(), value.to_string());
        self
    }

    fn with_file(mut self, path: &str, content: &str) -> Self {
        self.files.insert(path.to_string(), content.to_string());
        self
    }

    fn with_path(mut self, path: &str) -> Self {
        self.existing_paths.push(path.to_string());
        self
    }
}

impl ContextProvider for MockContextProviderRefined {
    fn git_output(&self, args: &[&str]) -> Result<String> {
        let key = args.join(" ");
        match self.git_outputs.get(&key) {
            Some(Ok(s)) => Ok(s.clone()),
            Some(Err(e)) => Err(anyhow!("{}", e)),
            None => Err(anyhow!("Mock git command not found: {}", key)),
        }
    }

    fn var(&self, key: &str) -> Result<String> {
        self.env_vars
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow!("Env var not found: {}", key))
    }

    fn read_to_string(&self, path: &str) -> Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow!("File not found: {}", path))
    }

    fn path_exists(&self, path: &str) -> bool {
        self.existing_paths.contains(&path.to_string())
    }
}

#[test]
fn test_intent_resolver_unknown_empty() {
    let resolver = IntentResolver::default();
    let ctx = IntentContext::default();
    let intent = resolver.resolve(&ctx);
    assert_eq!(intent.intent, IntentType::Unknown);
    // Base confidence 0.55 - 0.20 = 0.35
    assert!((intent.confidence - 0.35).abs() < 0.001);
}

#[test]
fn test_intent_resolver_coding_strong() {
    let resolver = IntentResolver::default();
    let mut ctx = IntentContext::default();
    ctx.changed_paths.push("crates/core/src/lib.rs".to_string());
    ctx.changed_paths.push("src/main.rs".to_string());

    let intent = resolver.resolve(&ctx);
    assert_eq!(intent.intent, IntentType::Coding);
    // Base 0.55 + 0.15 = 0.70
    assert!((intent.confidence - 0.70).abs() < 0.001);
}

#[test]
fn test_intent_resolver_writing() {
    let resolver = IntentResolver::default();
    let mut ctx = IntentContext::default();
    ctx.changed_paths.push("docs/README.md".to_string());
    ctx.changed_paths.push("docs/intent.md".to_string());

    let intent = resolver.resolve(&ctx);
    assert_eq!(intent.intent, IntentType::Writing);
}

#[test]
fn test_intent_resolver_mixed() {
    let resolver = IntentResolver::default();
    let mut ctx = IntentContext::default();
    // 1 coding
    ctx.changed_paths.push("crates/core/src/lib.rs".to_string());
    // 1 writing
    ctx.changed_paths.push("docs/README.md".to_string());

    let intent = resolver.resolve(&ctx);
    // Tie breaking: Coding > Writing
    assert_eq!(intent.intent, IntentType::Coding);

    // Mixed signals: Coding has 1, Writing has 1. Total 2.
    // Ratio for Coding: 0.5.
    // Rule: < 0.6 -> -0.20.
    // Base 0.55 - 0.20 = 0.35
    assert!((intent.confidence - 0.35).abs() < 0.001);
}

#[test]
fn test_intent_resolver_ci_triage() {
    let resolver = IntentResolver::default();
    let mut ctx = IntentContext::default();
    ctx.changed_paths
        .push(".github/workflows/ci.yml".to_string());

    let intent = resolver.resolve(&ctx);
    assert_eq!(intent.intent, IntentType::CiTriage);
}

#[test]
fn test_intent_resolver_pr_comments() {
    let resolver = IntentResolver::default();
    let mut ctx = IntentContext::default();
    ctx.pr_comments
        .push("Some comment /quick check".to_string());

    let intent = resolver.resolve(&ctx);
    assert_eq!(intent.intent, IntentType::CiTriage);
    // Comment adds weight 5 for CiTriage.
    // Signal count: 1. Ratio 1.0.
    // Base 0.55 + 0.15 = 0.70.
    assert!((intent.confidence - 0.70).abs() < 0.001);
}

#[test]
fn test_intent_resolver_contracts() {
    let resolver = IntentResolver::default();
    let mut ctx = IntentContext::default();
    ctx.changed_paths
        .push("contracts/event.line.json".to_string());

    let intent = resolver.resolve(&ctx);
    assert_eq!(intent.intent, IntentType::ContractsWork);
}

// --- New Tests for Context Gathering ---

#[test]
fn test_gather_context_no_git() {
    // Mock provider with no .git directory
    let provider = MockContextProviderRefined::new();
    // Note: We don't call .with_path(".git")

    let ctx = gather_context_with_provider(&provider).expect("Gather context should succeed");
    assert!(ctx.changed_paths.is_empty());
}

#[test]
fn test_gather_context_git_local_changes() {
    let provider = MockContextProviderRefined::new()
        .with_path(".git")
        .with_git_output(
            &["diff", "--name-only", "HEAD"],
            Ok("src/main.rs\ncrates/core/lib.rs".to_string()),
        )
        // Fallback or secondary call might happen, but existing logic calls diff HEAD first
        .with_git_output(
            &["diff", "--name-only", "origin/main...HEAD"],
            Err("Failed".to_string()),
        )
        .with_git_output(
            &["diff", "--name-only", "main...HEAD"],
            Err("Failed".to_string()),
        );

    let ctx = gather_context_with_provider(&provider).expect("Gather context should succeed");
    assert_eq!(ctx.changed_paths.len(), 2);
    assert!(ctx.changed_paths.contains(&"src/main.rs".to_string()));
    assert!(ctx
        .changed_paths
        .contains(&"crates/core/lib.rs".to_string()));
}

#[test]
fn test_gather_context_git_ci_changes() {
    let provider = MockContextProviderRefined::new()
        .with_path(".git")
        .with_git_output(
            &["diff", "--name-only", "HEAD"],
            Ok("".to_string()), // No uncommitted changes
        )
        .with_git_output(
            &["diff", "--name-only", "origin/main...HEAD"],
            Ok(".github/workflows/ci.yml".to_string()),
        );

    let ctx = gather_context_with_provider(&provider).expect("Gather context should succeed");
    assert_eq!(ctx.changed_paths.len(), 1);
    assert!(ctx
        .changed_paths
        .contains(&".github/workflows/ci.yml".to_string()));
}

#[test]
fn test_gather_context_git_fallback() {
    let provider = MockContextProviderRefined::new()
        .with_path(".git")
        .with_git_output(&["diff", "--name-only", "HEAD"], Ok("".to_string()))
        .with_git_output(
            &["diff", "--name-only", "origin/main...HEAD"],
            Err("ambiguous argument".to_string()),
        )
        .with_git_output(
            &["diff", "--name-only", "main...HEAD"],
            Ok("docs/README.md".to_string()),
        );

    let ctx = gather_context_with_provider(&provider).expect("Gather context should succeed");
    assert_eq!(ctx.changed_paths.len(), 1);
    assert!(ctx.changed_paths.contains(&"docs/README.md".to_string()));
}

#[test]
fn test_gather_context_workflow_env() {
    let provider = MockContextProviderRefined::new().with_env("GITHUB_WORKFLOW", "CI Pipeline");

    let ctx = gather_context_with_provider(&provider).expect("Gather context should succeed");
    assert_eq!(ctx.workflow_name, Some("CI Pipeline".to_string()));
}

#[test]
fn test_gather_context_pr_comments() {
    let event_path = "/tmp/event.json";
    let event_content = r#"{
        "comment": {
            "body": "/review please"
        }
    }"#;

    let provider = MockContextProviderRefined::new()
        .with_env("GITHUB_EVENT_PATH", event_path)
        .with_file(event_path, event_content);

    let ctx = gather_context_with_provider(&provider).expect("Gather context should succeed");
    assert_eq!(ctx.pr_comments.len(), 1);
    assert_eq!(ctx.pr_comments[0], "/review please");
}

#[test]
fn test_gather_context_pr_comments_invalid_json() {
    let event_path = "/tmp/event.json";
    let event_content = "invalid json";

    let provider = MockContextProviderRefined::new()
        .with_env("GITHUB_EVENT_PATH", event_path)
        .with_file(event_path, event_content);

    let ctx = gather_context_with_provider(&provider).expect("Gather context should succeed");
    assert!(ctx.pr_comments.is_empty());
}

#[test]
fn test_gather_context_deduplication() {
    // Scenario: File is both locally modified AND in the CI diff range (should appear once)
    let provider = MockContextProviderRefined::new()
        .with_path(".git")
        .with_git_output(
            &["diff", "--name-only", "HEAD"],
            Ok("src/common.rs".to_string()),
        )
        .with_git_output(
            &["diff", "--name-only", "origin/main...HEAD"],
            Ok("src/common.rs".to_string()),
        );

    let ctx = gather_context_with_provider(&provider).expect("Gather context should succeed");
    assert_eq!(ctx.changed_paths.len(), 1);
    assert_eq!(ctx.changed_paths[0], "src/common.rs");
}
