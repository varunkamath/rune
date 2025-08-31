use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info};

use crate::Config;

/// Manages embedding generation with caching and batch processing
pub struct EmbeddingGenerator {
    config: Arc<Config>,
    /// Cache embeddings by content hash to avoid recomputation
    cache: Arc<DashMap<String, Vec<f32>>>,
    dimension: usize,
}

impl EmbeddingGenerator {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let cache = Arc::new(DashMap::new());

        // For now, we'll use placeholder embeddings
        // In production, you would load a real model here
        let dimension = 256; // StarEncoder dimension

        info!(
            "Initialized embedding generator with dimension {}",
            dimension
        );

        Ok(Self {
            config,
            cache,
            dimension,
        })
    }

    /// Generate embedding for a single text
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Check cache first
        let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
        if let Some(cached) = self.cache.get(&hash) {
            debug!("Cache hit for embedding");
            return Ok(cached.clone());
        }

        // Generate a deterministic placeholder embedding based on text content
        // In production, this would use a real embedding model
        let mut embedding = vec![0.0; self.dimension];
        let text_hash = blake3::hash(text.as_bytes());
        let hash_bytes = text_hash.as_bytes();

        for (i, byte) in hash_bytes.iter().enumerate() {
            if i >= self.dimension {
                break;
            }
            embedding[i] = (*byte as f32) / 255.0 - 0.5; // Normalize to [-0.5, 0.5]
        }

        // Normalize the embedding
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        self.cache.insert(hash, embedding.clone());
        Ok(embedding)
    }

    /// Generate embeddings for multiple texts with batch processing
    pub async fn batch_generate(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            embeddings.push(self.generate_embedding(text).await?);
        }
        Ok(embeddings)
    }

    /// Check if the embedding model is available
    pub fn is_available(&self) -> bool {
        // For now, always available since we're using placeholder embeddings
        true
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}
