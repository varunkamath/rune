#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use rune_core::{
    Config, RuneEngine,
    search::{SearchMode, SearchQuery},
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[napi]
pub struct RuneBridge {
    engine: Arc<RwLock<Option<RuneEngine>>>,
}

#[napi]
impl RuneBridge {
    #[napi(constructor)]
    pub fn new() -> Result<Self> {
        Ok(Self {
            engine: Arc::new(RwLock::new(None)),
        })
    }

    #[napi]
    pub async fn initialize(&self, config_json: String) -> Result<()> {
        let config: ConfigJs = serde_json::from_str(&config_json)
            .map_err(|e| Error::from_reason(format!("Invalid config: {}", e)))?;

        let workspace_roots: Vec<PathBuf> = config
            .workspace_roots
            .into_iter()
            .map(PathBuf::from)
            .collect();

        let rust_config = Config {
            workspace_dir: workspace_roots
                .first()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string()),
            workspace_roots,
            cache_dir: PathBuf::from(config.cache_dir),
            max_file_size: config.max_file_size,
            indexing_threads: config.indexing_threads,
            enable_semantic: config.enable_semantic,
            languages: config.languages,
        };

        let engine = RuneEngine::new(rust_config)
            .await
            .map_err(|e| Error::from_reason(format!("Failed to initialize engine: {}", e)))?;

        let mut lock = self.engine.write().await;
        *lock = Some(engine);

        Ok(())
    }

    #[napi]
    pub async fn start(&self) -> Result<()> {
        let mut lock = self.engine.write().await;
        let engine = lock
            .as_mut()
            .ok_or_else(|| Error::from_reason("Engine not initialized"))?;

        engine
            .start()
            .await
            .map_err(|e| Error::from_reason(format!("Failed to start engine: {}", e)))?;

        Ok(())
    }

    #[napi]
    pub async fn stop(&self) -> Result<()> {
        let mut lock = self.engine.write().await;
        let engine = lock
            .as_mut()
            .ok_or_else(|| Error::from_reason("Engine not initialized"))?;

        engine
            .stop()
            .await
            .map_err(|e| Error::from_reason(format!("Failed to stop engine: {}", e)))?;

        Ok(())
    }

    #[napi]
    pub async fn search(&self, query_json: String) -> Result<String> {
        let lock = self.engine.read().await;
        let engine = lock
            .as_ref()
            .ok_or_else(|| Error::from_reason("Engine not initialized"))?;

        let query: SearchQueryJs = serde_json::from_str(&query_json)
            .map_err(|e| Error::from_reason(format!("Invalid query: {}", e)))?;

        let mode = match query.mode.as_str() {
            "literal" => SearchMode::Literal,
            "regex" => SearchMode::Regex,
            "symbol" => SearchMode::Symbol,
            "semantic" => SearchMode::Semantic,
            "hybrid" => SearchMode::Hybrid,
            _ => {
                return Err(Error::from_reason(format!(
                    "Invalid search mode: {}",
                    query.mode
                )));
            },
        };

        let rust_query = SearchQuery {
            query: query.query,
            mode,
            repositories: query.repositories,
            file_patterns: query.file_patterns,
            limit: query.limit,
            offset: query.offset,
        };

        let response = engine
            .search()
            .search(rust_query)
            .await
            .map_err(|e| Error::from_reason(format!("Search failed: {}", e)))?;

        serde_json::to_string(&response)
            .map_err(|e| Error::from_reason(format!("Failed to serialize response: {}", e)))
    }

    #[napi]
    pub async fn get_stats(&self) -> Result<String> {
        let lock = self.engine.read().await;
        let engine = lock
            .as_ref()
            .ok_or_else(|| Error::from_reason("Engine not initialized"))?;

        let stats = engine
            .stats()
            .await
            .map_err(|e| Error::from_reason(format!("Failed to get stats: {}", e)))?;

        serde_json::to_string(&stats)
            .map_err(|e| Error::from_reason(format!("Failed to serialize stats: {}", e)))
    }

    #[napi]
    pub async fn reindex(&self) -> Result<()> {
        let lock = self.engine.read().await;
        let engine = lock
            .as_ref()
            .ok_or_else(|| Error::from_reason("Engine not initialized"))?;

        engine
            .indexer()
            .reindex()
            .await
            .map_err(|e| Error::from_reason(format!("Reindex failed: {}", e)))?;

        Ok(())
    }

    /// Simple echo function for testing the bridge
    #[napi]
    pub async fn echo(&self, message: String) -> Result<String> {
        Ok(format!("Echo from Rust: {}", message))
    }
}

// JavaScript-compatible structs for serialization
#[derive(serde::Deserialize)]
struct ConfigJs {
    workspace_roots: Vec<String>,
    cache_dir: String,
    max_file_size: usize,
    indexing_threads: usize,
    enable_semantic: bool,
    languages: Vec<String>,
}

#[derive(serde::Deserialize)]
struct SearchQueryJs {
    query: String,
    mode: String,
    repositories: Option<Vec<String>>,
    file_patterns: Option<Vec<String>>,
    limit: usize,
    offset: usize,
}
