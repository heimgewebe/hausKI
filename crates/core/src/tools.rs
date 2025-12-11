use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(title = "ToolResult")]
pub struct ToolResult {
    pub tool_name: String,
    pub output: String,
    pub status: String,
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute<'a>(
        &'a self,
        input: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn list(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
        keys.sort();
        keys
    }
}

pub struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echos the input back to the caller."
    }

    fn execute<'a>(
        &'a self,
        input: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move { Ok(format!("Echo: {}", input)) })
    }
}

pub struct CodeAnalysisTool;

impl Tool for CodeAnalysisTool {
    fn name(&self) -> &str {
        "code_analysis"
    }

    fn description(&self) -> &str {
        "Analyzes the code snippet (Stub)."
    }

    fn execute<'a>(
        &'a self,
        _input: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            Ok("Code analysis tool is a stub in this MVP. Future: run linter/parser.".to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_tool() {
        let tool = EchoTool;
        let result = tool.execute("hello").await;
        assert_eq!(result.unwrap(), "Echo: hello");
    }

    #[tokio::test]
    async fn test_registry() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool));

        assert!(registry.get("echo").is_some());
        assert!(registry.get("nonexistent").is_none());
        assert_eq!(registry.list(), vec!["echo"]);
    }
}
