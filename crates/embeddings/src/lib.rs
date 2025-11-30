use anyhow::Result;
use serde::{Deserialize, Serialize};
use url::Url;

/// Trait for embedding providers.
pub trait Embedder {
    /// Creates embeddings for multiple texts.
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

/// Stub implementation for Ollama.
#[derive(Debug, Clone)]
pub struct OllamaEmbedder {
    base_url: Url,
    model: String,
}

#[derive(Debug, Serialize)]
pub struct OllamaEmbedRequest<'a> {
    pub model: &'a str,
    pub input: &'a [String],
}

#[derive(Debug, Deserialize)]
pub struct OllamaEmbedResponse {
    pub embeddings: Vec<Vec<f32>>,
}

impl OllamaEmbedder {
    pub fn new(base_url: Url, model: impl Into<String>) -> Self {
        Self {
            base_url,
            model: model.into(),
        }
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

impl Embedder for OllamaEmbedder {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        // Stub: returns empty vectors until HTTP integration is implemented.
        Ok(texts.iter().map(|_| Vec::new()).collect())
    }
}
