//! Embedding-Modul für HausKI.
//!
//! Dieses Modul stellt Traits und Implementierungen für Text-Embeddings bereit,
//! die für semantische Suche und Ähnlichkeitsvergleiche genutzt werden.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use url::Url;

/// Trait für Embedding-Anbieter.
pub trait Embedder {
    /// Erstellt Embeddings für mehrere Texte.
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

/// Client-Implementierung für Ollama-Embeddings.
///
/// Diese Implementierung kommuniziert mit einem lokalen Ollama-Server
/// für die Generierung von Text-Embeddings.
#[derive(Debug, Clone)]
pub struct OllamaEmbedder {
    base_url: Url,
    model: String,
}

/// Request-Struktur für Ollama-Embedding-API.
#[derive(Debug, Serialize)]
pub struct OllamaEmbedRequest<'a> {
    /// Name des zu verwendenden Modells.
    pub model: &'a str,
    /// Liste der Texte, für die Embeddings erstellt werden sollen.
    pub input: &'a [String],
}

/// Response-Struktur von Ollama-Embedding-API.
#[derive(Debug, Deserialize)]
pub struct OllamaEmbedResponse {
    /// Liste der Embedding-Vektoren.
    pub embeddings: Vec<Vec<f32>>,
}

impl OllamaEmbedder {
    /// Erstellt einen neuen OllamaEmbedder.
    ///
    /// # Argumente
    ///
    /// * `base_url` - Basis-URL des Ollama-Servers
    /// * `model` - Name des zu verwendenden Embedding-Modells
    pub fn new(base_url: Url, model: impl Into<String>) -> Self {
        Self {
            base_url,
            model: model.into(),
        }
    }

    /// Gibt die Basis-URL des Embedders zurück.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Gibt den Modellnamen zurück.
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
