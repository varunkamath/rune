#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use rune_core::{
    Config, RuneEngine,
    search::{SearchMode, SearchQuery},
};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

// Helper to suppress stdout during Qdrant operations
struct StdoutSuppressor {
    saved_stdout: Option<std::fs::File>,
}

impl StdoutSuppressor {
    fn new() -> Self {
        unsafe {
            // Save current stdout
            let saved = libc::dup(1);
            if saved != -1 {
                // Redirect stdout to /dev/null
                let devnull = std::fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/null")
                    .ok();
                if let Some(null) = devnull {
                    libc::dup2(null.as_raw_fd(), 1);
                    return StdoutSuppressor {
                        saved_stdout: Some(std::fs::File::from_raw_fd(saved)),
                    };
                }
            }
        }
        StdoutSuppressor { saved_stdout: None }
    }
}

impl Drop for StdoutSuppressor {
    fn drop(&mut self) {
        if let Some(saved) = self.saved_stdout.take() {
            unsafe {
                // Restore stdout
                libc::dup2(saved.as_raw_fd(), 1);
            }
        }
    }
}

#[napi]
pub struct RuneBridge {
    engine: Arc<RwLock<Option<RuneEngine>>>,
}

#[napi]
impl RuneBridge {
    #[napi(constructor)]
    pub fn new() -> Result<Self> {
        // Suppress all logging to prevent stdout pollution
        // The Qdrant client library prints directly to stdout, breaking JSON-RPC
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            // Set environment to suppress Qdrant client output
            unsafe {
                std::env::set_var("RUST_LOG", "off");
            }

            // Initialize a null subscriber that drops all tracing events
            struct NullSubscriber;
            impl tracing::Subscriber for NullSubscriber {
                fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
                    false
                }
                fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
                    tracing::span::Id::from_u64(1)
                }
                fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}
                fn record_follows_from(
                    &self,
                    _span: &tracing::span::Id,
                    _follows: &tracing::span::Id,
                ) {
                }
                fn event(&self, _event: &tracing::Event<'_>) {}
                fn enter(&self, _span: &tracing::span::Id) {}
                fn exit(&self, _span: &tracing::span::Id) {}
            }

            let _ = tracing::subscriber::set_global_default(NullSubscriber);
        });

        Ok(Self {
            engine: Arc::new(RwLock::new(None)),
        })
    }

    #[napi]
    pub async fn initialize(&self, config_json: String) -> Result<()> {
        // Suppress stdout for Qdrant client warnings
        let _guard = StdoutSuppressor::new();

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
            file_watch_debounce_ms: config.file_watch_debounce_ms,
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
        // Suppress stdout for Qdrant client warnings during start
        let _guard = StdoutSuppressor::new();

        let mut lock = self.engine.write().await;
        let engine = lock
            .as_mut()
            .ok_or_else(|| Error::from_reason("Engine not initialized"))?;

        engine
            .start()
            .await
            .map_err(|e| Error::from_reason(format!("Failed to start engine: {}", e)))?;

        // Trigger initial indexing
        engine
            .indexer()
            .reindex()
            .await
            .map_err(|e| Error::from_reason(format!("Failed to reindex: {}", e)))?;

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

        let mode = match query.mode.to_lowercase().as_str() {
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
            query: query.query.clone(),
            mode,
            repositories: query.repositories.clone(),
            file_patterns: query.file_patterns.clone(),
            limit: query.limit,
            offset: query.offset,
        };

        let response = engine
            .search()
            .search(rust_query)
            .await
            .map_err(|e| Error::from_reason(format!("Search failed: {}", e)))?;

        let json_response = serde_json::to_string(&response)
            .map_err(|e| Error::from_reason(format!("Failed to serialize response: {}", e)))?;

        Ok(json_response)
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

        // Add watching status to stats
        let mut stats_json = serde_json::to_value(&stats)
            .map_err(|e| Error::from_reason(format!("Failed to serialize stats: {}", e)))?;

        if let Some(obj) = stats_json.as_object_mut() {
            obj.insert(
                "file_watching_active".to_string(),
                serde_json::Value::Bool(engine.is_watching()),
            );
        }

        serde_json::to_string(&stats_json)
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
    file_watch_debounce_ms: u64,
}

#[derive(serde::Deserialize, Debug)]
struct SearchQueryJs {
    query: String,
    mode: String,
    repositories: Option<Vec<String>>,
    file_patterns: Option<Vec<String>>,
    limit: usize,
    offset: usize,
}
