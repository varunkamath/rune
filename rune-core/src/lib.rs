#![allow(dead_code)] // TODO: Remove when implementation is complete

pub mod error;
pub mod indexing;
pub mod search;
pub mod storage;

#[cfg(feature = "semantic")]
pub mod embedding;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

pub use error::RuneError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Workspace root directories
    pub workspace_roots: Vec<PathBuf>,

    /// Main workspace directory (first workspace root)
    pub workspace_dir: String,

    /// Cache directory for indexes and embeddings
    pub cache_dir: PathBuf,

    /// Maximum file size to index (in bytes)
    pub max_file_size: usize,

    /// Number of worker threads for indexing
    pub indexing_threads: usize,

    /// Enable semantic search
    pub enable_semantic: bool,

    /// Languages to support
    pub languages: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let workspace_roots = vec![PathBuf::from(".")];
        let workspace_dir = workspace_roots[0].to_string_lossy().to_string();
        Self {
            workspace_roots,
            workspace_dir,
            cache_dir: PathBuf::from(".rune_cache"),
            max_file_size: 10 * 1024 * 1024, // 10MB
            indexing_threads: num_cpus::get(),
            enable_semantic: true,
            languages: vec![
                "rust".to_string(),
                "javascript".to_string(),
                "typescript".to_string(),
                "python".to_string(),
                "go".to_string(),
                "java".to_string(),
                "cpp".to_string(),
            ],
        }
    }
}

/// Main engine for the Rune code search system
pub struct RuneEngine {
    config: Arc<Config>,
    search_engine: search::SearchEngine,
    indexer: indexing::Indexer,
    storage: storage::StorageBackend,
}

impl RuneEngine {
    /// Create a new Rune engine with the given configuration
    pub async fn new(config: Config) -> Result<Self> {
        info!(
            "Initializing Rune engine with {} workspace roots",
            config.workspace_roots.len()
        );

        let config = Arc::new(config);

        // Initialize storage backend
        let storage = storage::StorageBackend::new(&config.cache_dir).await?;

        // Initialize search engine
        let search_engine = search::SearchEngine::new(config.clone(), storage.clone()).await?;

        // Initialize indexer
        let indexer = indexing::Indexer::new(config.clone(), storage.clone()).await?;

        Ok(Self {
            config,
            search_engine,
            indexer,
            storage,
        })
    }

    /// Start the engine (begins file watching and indexing)
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Rune engine");

        // Start file watcher
        self.indexer.start_watching().await?;

        // Initial index of workspace
        self.indexer.index_workspaces().await?;

        Ok(())
    }

    /// Stop the engine
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Rune engine");
        self.indexer.stop_watching().await?;
        Ok(())
    }

    /// Get the search engine
    pub fn search(&self) -> &search::SearchEngine {
        &self.search_engine
    }

    /// Get the indexer
    pub fn indexer(&self) -> &indexing::Indexer {
        &self.indexer
    }

    /// Get engine statistics
    pub async fn stats(&self) -> Result<EngineStats> {
        Ok(EngineStats {
            indexed_files: self.storage.get_file_count().await?,
            total_symbols: self.storage.get_symbol_count().await?,
            index_size_bytes: self.storage.get_index_size().await?,
            cache_size_bytes: self.storage.get_cache_size().await?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EngineStats {
    pub indexed_files: usize,
    pub total_symbols: usize,
    pub index_size_bytes: u64,
    pub cache_size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_engine_creation() {
        let tmp_dir = tempdir().unwrap();
        let config = Config {
            workspace_roots: vec![tmp_dir.path().to_path_buf()],
            cache_dir: tmp_dir.path().join(".cache"),
            ..Default::default()
        };

        let engine = RuneEngine::new(config).await;
        // Print error if creation failed
        if let Err(ref e) = engine {
            eprintln!("Engine creation failed: {}", e);
        }
        assert!(engine.is_ok());
    }
}
