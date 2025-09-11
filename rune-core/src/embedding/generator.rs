use anyhow::{Context, Result};
use dashmap::DashMap;
use ndarray::Array2;
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;
use tracing::{debug, info, warn};

use super::model_manager::ModelManager;
use crate::Config;

/// Manages embedding generation using ONNX Runtime with caching and batch processing
pub struct EmbeddingGenerator {
    _config: Arc<Config>, // Kept for potential future configuration needs
    session: Option<Arc<Mutex<Session>>>,
    tokenizer: Option<Arc<Tokenizer>>,
    /// Cache embeddings by content hash to avoid recomputation
    cache: Arc<DashMap<String, Vec<f32>>>,
    dimension: usize,
    fallback_mode: bool,
}

impl EmbeddingGenerator {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let cache = Arc::new(DashMap::new());

        // Try to initialize ONNX model
        match Self::initialize_model(&config).await {
            Ok((session, tokenizer)) => {
                info!("Successfully initialized all-MiniLM-L6-v2 model (384 dimensions)");
                Ok(Self {
                    _config: config,
                    session: Some(Arc::new(Mutex::new(session))),
                    tokenizer: Some(Arc::new(tokenizer)),
                    cache,
                    dimension: 384,
                    fallback_mode: false,
                })
            },
            Err(e) => {
                warn!(
                    "Failed to initialize ONNX model: {}. Using fallback mode.",
                    e
                );
                Ok(Self {
                    _config: config,
                    session: None,
                    tokenizer: None,
                    cache,
                    dimension: 256, // Fallback dimension
                    fallback_mode: true,
                })
            },
        }
    }

    async fn initialize_model(config: &Arc<Config>) -> Result<(Session, Tokenizer)> {
        // Get model path using ModelManager
        let model_manager = ModelManager::with_cache_dir(config.cache_dir.clone());

        let model_path = model_manager
            .get_model_path()
            .await
            .context("Failed to get model path")?;

        // Initialize ONNX session with ORT v2 API
        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("Failed to create session builder: {:?}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("Failed to set optimization level: {:?}", e))?
            .with_intra_threads(num_cpus::get())
            .map_err(|e| anyhow::anyhow!("Failed to set intra threads: {:?}", e))?
            .commit_from_file(model_path.join("model.onnx"))
            .map_err(|e| anyhow::anyhow!("Failed to load model from file: {:?}", e))?;

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(model_path.join("tokenizer.json"))
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        Ok((session, tokenizer))
    }

    /// Generate embedding for a single text
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Check cache first
        let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
        if let Some(cached) = self.cache.get(&hash) {
            debug!("Cache hit for embedding");
            return Ok(cached.clone());
        }

        let embedding = if self.fallback_mode {
            self.generate_fallback_embedding(text)?
        } else {
            self.generate_onnx_embedding(text).await?
        };

        self.cache.insert(hash, embedding.clone());
        Ok(embedding)
    }

    /// Generate embedding using ONNX model
    async fn generate_onnx_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let session = self
            .session
            .as_ref()
            .context("ONNX session not initialized")?;
        let tokenizer = self
            .tokenizer
            .as_ref()
            .context("Tokenizer not initialized")?;

        // Tokenize the text
        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let seq_len = input_ids.len();

        // Convert to ndarray for ONNX
        let input_ids_array = Array2::from_shape_vec(
            (1, seq_len),
            input_ids.iter().map(|&id| id as i64).collect(),
        )?;

        let attention_mask_array = Array2::from_shape_vec(
            (1, seq_len),
            attention_mask.iter().map(|&m| m as i64).collect(),
        )?;

        // Prepare token_type_ids (all zeros for single sequence)
        let token_type_ids_array = Array2::<i64>::zeros((1, seq_len));

        // Create tensors from arrays (ORT v2 API)
        let input_ids_tensor = Tensor::from_array(input_ids_array)
            .map_err(|e| anyhow::anyhow!("Failed to create input_ids tensor: {:?}", e))?;
        let attention_mask_tensor = Tensor::from_array(attention_mask_array)
            .map_err(|e| anyhow::anyhow!("Failed to create attention_mask tensor: {:?}", e))?;
        let token_type_ids_tensor = Tensor::from_array(token_type_ids_array)
            .map_err(|e| anyhow::anyhow!("Failed to create token_type_ids tensor: {:?}", e))?;

        // Run inference with named inputs and extract embeddings
        let embeddings_view = {
            let mut session_guard = session.lock().unwrap();
            let outputs = session_guard
                .run(ort::inputs![
                    "input_ids" => input_ids_tensor,
                    "attention_mask" => attention_mask_tensor,
                    "token_type_ids" => token_type_ids_tensor
                ])
                .map_err(|e| anyhow::anyhow!("Failed to run inference: {:?}", e))?;

            // Extract embeddings - for all-MiniLM-L6-v2, the output is "last_hidden_state"
            let embeddings_view: ndarray::ArrayViewD<f32> = outputs["last_hidden_state"]
                .try_extract_array()
                .map_err(|e| anyhow::anyhow!("Failed to extract embeddings tensor: {:?}", e))?;

            // Clone the data to own it before dropping the lock
            embeddings_view.to_owned()
        };

        // Apply mean pooling
        let pooled = self.mean_pool_ndarray(embeddings_view.view(), attention_mask);

        // L2 normalize for cosine similarity
        let normalized = self.l2_normalize(pooled);

        Ok(normalized)
    }

    /// Apply mean pooling to embeddings using ndarray
    fn mean_pool_ndarray(
        &self,
        embeddings: ndarray::ArrayViewD<f32>,
        attention_mask: &[u32],
    ) -> Vec<f32> {
        // embeddings shape should be [1, seq_len, hidden_size]
        let shape = embeddings.shape();
        if shape.len() != 3 {
            // Fallback to zeros if unexpected shape
            return vec![0.0; 384];
        }

        let seq_len = shape[1];
        let hidden_size = shape[2];

        let mut pooled = vec![0.0; hidden_size];
        let mut valid_tokens = 0.0;

        for i in 0..seq_len.min(attention_mask.len()) {
            if attention_mask[i] == 1 {
                valid_tokens += 1.0;
                for j in 0..hidden_size {
                    pooled[j] += embeddings[[0, i, j]];
                }
            }
        }

        if valid_tokens > 0.0 {
            for val in &mut pooled {
                *val /= valid_tokens;
            }
        }

        pooled
    }

    /// L2 normalize a vector
    fn l2_normalize(&self, mut vec: Vec<f32>) -> Vec<f32> {
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut vec {
                *val /= norm;
            }
        }
        vec
    }

    /// Generate a fallback embedding (same as before for compatibility)
    fn generate_fallback_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let mut embedding = vec![0.0; self.dimension];
        let text_hash = blake3::hash(text.as_bytes());
        let hash_bytes = text_hash.as_bytes();

        for (i, byte) in hash_bytes.iter().enumerate() {
            if i >= self.dimension {
                break;
            }
            embedding[i] = (*byte as f32) / 255.0 - 0.5;
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        Ok(embedding)
    }

    /// Generate embeddings for multiple texts with batch processing
    pub async fn batch_generate(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        if self.fallback_mode {
            // Fallback mode: generate individually
            let mut embeddings = Vec::with_capacity(texts.len());
            for text in texts {
                embeddings.push(self.generate_embedding(text).await?);
            }
            return Ok(embeddings);
        }

        // ONNX batch processing
        const BATCH_SIZE: usize = 32;
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(BATCH_SIZE) {
            let batch_embeddings = self.batch_generate_onnx(chunk).await?;
            all_embeddings.extend(batch_embeddings);
        }

        Ok(all_embeddings)
    }

    /// Batch generate embeddings using ONNX
    async fn batch_generate_onnx(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let session = self
            .session
            .as_ref()
            .context("ONNX session not initialized")?;
        let tokenizer = self
            .tokenizer
            .as_ref()
            .context("Tokenizer not initialized")?;

        // Tokenize all texts and find max length
        let mut encodings = Vec::new();
        let mut max_len = 0;

        for text in texts {
            let encoding = tokenizer
                .encode(text.as_str(), true)
                .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
            max_len = max_len.max(encoding.len());
            encodings.push(encoding);
        }

        // Pad sequences to max length
        let batch_size = texts.len();
        let mut input_ids = Vec::with_capacity(batch_size * max_len);
        let mut attention_mask = Vec::with_capacity(batch_size * max_len);
        let mut token_type_ids = Vec::with_capacity(batch_size * max_len);

        for encoding in &encodings {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();

            // Add actual tokens
            input_ids.extend(ids.iter().map(|&id| id as i64));
            attention_mask.extend(mask.iter().map(|&m| m as i64));
            token_type_ids.extend(vec![0i64; ids.len()]);

            // Pad to max length
            let pad_len = max_len - ids.len();
            input_ids.extend(vec![0i64; pad_len]);
            attention_mask.extend(vec![0i64; pad_len]);
            token_type_ids.extend(vec![0i64; pad_len]);
        }

        // Convert to ndarray
        let input_ids_array = Array2::from_shape_vec((batch_size, max_len), input_ids)?;
        let attention_mask_array = Array2::from_shape_vec((batch_size, max_len), attention_mask)?;
        let token_type_ids_array = Array2::from_shape_vec((batch_size, max_len), token_type_ids)?;

        // Create tensors
        let input_ids_tensor = Tensor::from_array(input_ids_array)
            .map_err(|e| anyhow::anyhow!("Failed to create batch input_ids tensor: {:?}", e))?;
        let attention_mask_tensor =
            Tensor::from_array(attention_mask_array.clone()).map_err(|e| {
                anyhow::anyhow!("Failed to create batch attention_mask tensor: {:?}", e)
            })?;
        let token_type_ids_tensor = Tensor::from_array(token_type_ids_array).map_err(|e| {
            anyhow::anyhow!("Failed to create batch token_type_ids tensor: {:?}", e)
        })?;

        // Run batch inference and extract embeddings
        let embeddings_array = {
            let mut session_guard = session.lock().unwrap();
            let outputs = session_guard
                .run(ort::inputs![
                    "input_ids" => input_ids_tensor,
                    "attention_mask" => attention_mask_tensor,
                    "token_type_ids" => token_type_ids_tensor
                ])
                .map_err(|e| anyhow::anyhow!("Failed to run batch inference: {:?}", e))?;

            // Extract and process embeddings for each text
            let embeddings_view: ndarray::ArrayViewD<f32> = outputs["last_hidden_state"]
                .try_extract_array()
                .map_err(|e| {
                    anyhow::anyhow!("Failed to extract batch embeddings tensor: {:?}", e)
                })?;

            // Clone the data to own it before dropping the lock
            embeddings_view.to_owned()
        };

        let embeddings_view = embeddings_array.view();

        let hidden_size = 384;
        let mut result = Vec::with_capacity(batch_size);

        for (i, encoding) in encodings.iter().enumerate() {
            let mask = encoding.get_attention_mask();
            let text_len = encoding.len();

            // Extract embeddings for this text in the batch
            let mut pooled = vec![0.0; hidden_size];
            let mut valid_tokens = 0.0;

            for j in 0..text_len {
                if mask[j] == 1 {
                    valid_tokens += 1.0;
                    for k in 0..hidden_size {
                        pooled[k] += embeddings_view[[i, j, k]];
                    }
                }
            }

            if valid_tokens > 0.0 {
                for val in &mut pooled {
                    *val /= valid_tokens;
                }
            }

            let normalized = self.l2_normalize(pooled);
            result.push(normalized);
        }

        Ok(result)
    }

    /// Check if the embedding model is available
    pub fn is_available(&self) -> bool {
        !self.fallback_mode
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_fallback_mode() {
        let temp_dir = TempDir::new().unwrap();
        let config = Arc::new(Config {
            workspace_dir: temp_dir.path().to_string_lossy().to_string(),
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        });

        let generator = EmbeddingGenerator::new(config).await.unwrap();

        // Should work even in fallback mode
        let embedding = generator.generate_embedding("test text").await.unwrap();
        assert!(!embedding.is_empty());

        // Check normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_batch_generation() {
        let temp_dir = TempDir::new().unwrap();
        let config = Arc::new(Config {
            workspace_dir: temp_dir.path().to_string_lossy().to_string(),
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        });

        let generator = EmbeddingGenerator::new(config).await.unwrap();

        let texts = vec![
            "function foo() { return 42; }".to_string(),
            "class Bar { constructor() {} }".to_string(),
            "const x = 10;".to_string(),
        ];

        let embeddings = generator.batch_generate(&texts).await.unwrap();
        assert_eq!(embeddings.len(), texts.len());

        for embedding in embeddings {
            assert!(!embedding.is_empty());
            // Check normalization
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 0.01);
        }
    }

    #[tokio::test]
    #[ignore] // Run with --ignored to test actual model download
    async fn test_real_model_download() {
        use std::path::PathBuf;
        use tracing_subscriber;
        let _ = tracing_subscriber::fmt::try_init();

        let config = Arc::new(Config {
            workspace_dir: ".".to_string(),
            cache_dir: PathBuf::from(".rune_cache"),
            ..Default::default()
        });

        eprintln!("Attempting to download and initialize real model...");
        let generator = EmbeddingGenerator::new(config).await.unwrap();

        assert!(
            !generator.fallback_mode,
            "Should not be in fallback mode with real model"
        );
        assert_eq!(
            generator.dimension, 384,
            "Should have 384 dimensions for all-MiniLM-L6-v2"
        );

        // Test with real embeddings
        let text = "function test() { return 42; }";
        let embedding = generator.generate_embedding(text).await.unwrap();
        assert_eq!(embedding.len(), 384, "Embedding should have 384 dimensions");

        eprintln!("Successfully generated 384-dimensional embedding!");
    }

    #[tokio::test]
    async fn test_semantic_similarity() {
        let temp_dir = TempDir::new().unwrap();
        let config = Arc::new(Config {
            workspace_dir: temp_dir.path().to_string_lossy().to_string(),
            cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        });

        let generator = EmbeddingGenerator::new(config).await.unwrap();

        // Test semantic similarity between similar and dissimilar texts
        let similar1 = "function add(a, b) { return a + b; }";
        let similar2 = "function sum(x, y) { return x + y; }";
        let different = "The weather is nice today.";

        let emb1 = generator.generate_embedding(similar1).await.unwrap();
        let emb2 = generator.generate_embedding(similar2).await.unwrap();
        let emb3 = generator.generate_embedding(different).await.unwrap();

        // Calculate cosine similarities
        let sim_12 = cosine_similarity(&emb1, &emb2);
        let sim_13 = cosine_similarity(&emb1, &emb3);

        // Similar code should have higher similarity than different content
        // Note: In fallback mode, this test might not pass as expected
        if !generator.fallback_mode {
            assert!(
                sim_12 > sim_13,
                "Similar code should have higher similarity"
            );
        }
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 0.0 && norm_b > 0.0 {
            dot_product / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}
