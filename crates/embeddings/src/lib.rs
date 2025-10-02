use anyhow::Result;
use serde::{Deserialize, Serialize};
use url::Url;

/// Trait für Embedding-Anbieter.
pub trait Embedder {
    /// Erstellt Embeddings für mehrere Texte.
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

/// Stub-Implementierung für Ollama.
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
        // Stub: liefert leere Vektoren, bis die HTTP-Integration steht.
        Ok(texts.iter().map(|_| Vec::new()).collect())
    }
}
