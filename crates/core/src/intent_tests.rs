#[cfg(test)]
mod tests {
    use crate::intent::{IntentContext, IntentResolver, IntentType};

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
        ctx.changed_paths.push(".github/workflows/ci.yml".to_string());

        let intent = resolver.resolve(&ctx);
        assert_eq!(intent.intent, IntentType::CiTriage);
    }

    #[test]
    fn test_intent_resolver_pr_comments() {
        let resolver = IntentResolver::default();
        let mut ctx = IntentContext::default();
        ctx.pr_comments.push("Some comment /quick check".to_string());

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
        ctx.changed_paths.push("contracts/event.line.json".to_string());

        let intent = resolver.resolve(&ctx);
        assert_eq!(intent.intent, IntentType::ContractsWork);
    }
}
