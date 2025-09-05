pub mod ast_chunker;
pub mod chunker;
pub mod generator;
pub mod model_manager;
pub mod qdrant;
pub mod quantization;

pub use chunker::{ChunkType, ChunkerConfig, CodeChunk, CodeChunker};
pub use generator::EmbeddingGenerator;
pub use qdrant::{EmbeddedChunk, QdrantManager, SemanticSearchResult};

use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info};

use crate::Config;

/// High-level embedding pipeline that coordinates chunking, generation, and storage
pub struct EmbeddingPipeline {
    generator: Arc<EmbeddingGenerator>,
    qdrant: Arc<QdrantManager>,
    chunker: Arc<tokio::sync::Mutex<CodeChunker>>,
}

impl EmbeddingPipeline {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let generator = Arc::new(EmbeddingGenerator::new(config.clone()).await?);
        let qdrant = Arc::new(QdrantManager::new(config.clone()).await?);
        let chunker = Arc::new(tokio::sync::Mutex::new(CodeChunker::new(
            ChunkerConfig::default(),
        )));

        Ok(Self {
            generator,
            qdrant,
            chunker,
        })
    }

    /// Process a file and store its embeddings
    pub async fn process_file(&self, file_path: &str, content: &str) -> Result<()> {
        if !self.is_available() {
            debug!("Embedding pipeline not available, skipping file");
            return Ok(());
        }

        info!("Processing file for embeddings: {}", file_path);

        // Chunk the file
        let chunks = {
            let mut chunker = self.chunker.lock().await;
            chunker.chunk_file(content, file_path)
        };
        if chunks.is_empty() {
            return Ok(());
        }

        info!("Processing {} chunks for {}", chunks.len(), file_path);

        // Generate embeddings in batches
        let batch_size = 32;
        let mut embedded_chunks = Vec::new();

        for batch in chunks.chunks(batch_size) {
            let texts: Vec<String> = batch.iter().map(|c| c.content.clone()).collect();
            let embeddings = self.generator.batch_generate(&texts).await?;

            for (chunk, embedding) in batch.iter().zip(embeddings.iter()) {
                // Generate a deterministic UUID based on file path and content
                // This ensures the same chunk always gets the same ID, preventing duplicates
                let file_hash = blake3::hash(file_path.as_bytes());
                let content_hash = blake3::hash(chunk.content.as_bytes());
                let line_info = format!("{:08x}{:08x}", chunk.start_line, chunk.end_line);
                let combined = format!(
                    "{}{}{}",
                    &file_hash.to_hex()[..16],
                    line_info,
                    &content_hash.to_hex()[..8]
                );

                // Create a valid UUID format from our deterministic hash
                // Format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx (8-4-4-4-12)
                let id = format!(
                    "{}-{}-{}-{}-{}",
                    &combined[0..8],
                    &combined[8..12],
                    &combined[12..16],
                    &combined[16..20],
                    &combined[20..32]
                );

                embedded_chunks.push(EmbeddedChunk {
                    id,
                    content: chunk.content.clone(),
                    embedding: embedding.clone(),
                    file_path: chunk.file_path.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    language: chunk.language.clone(),
                });
            }
        }

        // Store in Qdrant
        self.qdrant.store_embeddings(embedded_chunks).await?;

        Ok(())
    }

    /// Search for semantically similar code
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SemanticSearchResult>> {
        if !self.is_available() {
            debug!("Embedding pipeline not available");
            return Ok(Vec::new());
        }

        // Generate query embedding
        let query_embedding = self.generator.generate_embedding(query).await?;

        // Search in Qdrant
        self.qdrant.search(query_embedding, limit, None).await
    }

    /// Check if the pipeline is fully operational
    pub fn is_available(&self) -> bool {
        self.generator.is_available() && self.qdrant.is_available()
    }

    /// Clear all stored embeddings
    pub async fn clear(&self) -> Result<()> {
        self.qdrant.clear_collection().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn create_test_config() -> Arc<Config> {
        Arc::new(Config {
            workspace_roots: vec![tempdir().unwrap().path().to_path_buf()],
            workspace_dir: tempdir().unwrap().path().to_string_lossy().to_string(),
            cache_dir: tempdir().unwrap().path().to_path_buf(),
            max_file_size: 10 * 1024 * 1024,
            indexing_threads: 1,
            enable_semantic: true,
            languages: vec!["rust".to_string(), "python".to_string()],
        })
    }

    #[tokio::test]
    async fn test_embedding_pipeline_initialization() {
        // Disable semantic to ensure pipeline handles it gracefully
        unsafe {
            std::env::set_var("RUNE_ENABLE_SEMANTIC", "false");
        }

        let config = create_test_config();
        let pipeline = EmbeddingPipeline::new(config).await.unwrap();

        // Pipeline should be created but not available without Qdrant
        assert!(!pipeline.is_available());

        unsafe {
            std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        }
    }

    #[tokio::test]
    async fn test_process_file_without_qdrant() {
        unsafe {
            std::env::set_var("RUNE_ENABLE_SEMANTIC", "false");
        }

        let config = create_test_config();
        let pipeline = EmbeddingPipeline::new(config).await.unwrap();

        let code = r#"
        fn main() {
            println!("Hello, world!");
        }
        "#;

        // Should handle gracefully without Qdrant
        pipeline.process_file("test.rs", code).await.unwrap();

        unsafe {
            std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        }
    }

    #[tokio::test]
    async fn test_search_without_qdrant() {
        unsafe {
            std::env::set_var("RUNE_ENABLE_SEMANTIC", "false");
        }

        let config = create_test_config();
        let pipeline = EmbeddingPipeline::new(config).await.unwrap();

        let results = pipeline.search("test query", 10).await.unwrap();
        assert_eq!(results.len(), 0);

        unsafe {
            std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        }
    }

    #[tokio::test]
    async fn test_clear_without_qdrant() {
        unsafe {
            std::env::set_var("RUNE_ENABLE_SEMANTIC", "false");
        }

        let config = create_test_config();
        let pipeline = EmbeddingPipeline::new(config).await.unwrap();

        // Should not panic
        pipeline.clear().await.unwrap();

        unsafe {
            std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        }
    }

    #[tokio::test]
    async fn test_process_multiple_languages() {
        unsafe {
            std::env::set_var("RUNE_ENABLE_SEMANTIC", "false");
        }

        let config = create_test_config();
        let pipeline = EmbeddingPipeline::new(config).await.unwrap();

        // Test Rust code
        let rust_code = r#"
        struct Person {
            name: String,
            age: u32,
        }
        "#;
        pipeline.process_file("person.rs", rust_code).await.unwrap();

        // Test Python code
        let python_code = r#"
        class Person:
            def __init__(self, name, age):
                self.name = name
                self.age = age
        "#;
        pipeline
            .process_file("person.py", python_code)
            .await
            .unwrap();

        // Test JavaScript code
        let js_code = r#"
        class Person {
            constructor(name, age) {
                this.name = name;
                this.age = age;
            }
        }
        "#;
        pipeline.process_file("person.js", js_code).await.unwrap();

        unsafe {
            std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        }
    }

    #[tokio::test]
    async fn test_process_empty_file() {
        unsafe {
            std::env::set_var("RUNE_ENABLE_SEMANTIC", "false");
        }

        let config = create_test_config();
        let pipeline = EmbeddingPipeline::new(config).await.unwrap();

        // Should handle empty content gracefully
        pipeline.process_file("empty.rs", "").await.unwrap();

        unsafe {
            std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        }
    }

    #[tokio::test]
    async fn test_process_large_file() {
        unsafe {
            std::env::set_var("RUNE_ENABLE_SEMANTIC", "false");
        }

        let config = create_test_config();
        let pipeline = EmbeddingPipeline::new(config).await.unwrap();

        // Generate a large file content
        let mut large_content = String::new();
        for i in 0..100 {
            large_content.push_str(&format!(
                "fn function_{i}() {{\n    println!(\"Function {i}\");\n}}\n\n"
            ));
        }

        // Should handle large files
        pipeline
            .process_file("large.rs", &large_content)
            .await
            .unwrap();

        unsafe {
            std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        }
    }

    #[tokio::test]
    async fn test_search_with_special_characters() {
        unsafe {
            std::env::set_var("RUNE_ENABLE_SEMANTIC", "false");
        }

        let config = create_test_config();
        let pipeline = EmbeddingPipeline::new(config).await.unwrap();

        // Test queries with special characters
        let queries = vec![
            "async/await",
            "Result<T, E>",
            "#[derive(Debug)]",
            "impl<'a> Iterator",
            "fn() -> ()",
        ];

        for query in queries {
            let results = pipeline.search(query, 5).await.unwrap();
            assert_eq!(results.len(), 0); // Without Qdrant, should return empty
        }

        unsafe {
            std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        }
    }
}
