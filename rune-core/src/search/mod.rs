pub mod semantic;
pub mod symbol;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    Config,
    cache::{CacheConfig, MultiTierCache},
    indexing::tantivy_indexer::TantivyIndexer,
    storage::StorageBackend,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchMode {
    Symbol,
    Semantic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub mode: SearchMode,
    pub repositories: Option<Vec<String>>,
    pub file_patterns: Option<Vec<String>>,
    pub limit: usize,
    pub offset: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            mode: SearchMode::Semantic,
            repositories: None,
            file_patterns: None,
            limit: 50,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: PathBuf,
    pub repository: String,
    pub line_number: usize,
    pub column: usize,
    pub content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
    pub score: f32,
    pub match_type: MatchType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchType {
    Semantic,
    Symbol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: SearchQuery,
    pub results: Vec<SearchResult>,
    pub total_matches: usize,
    pub search_time_ms: u64,
    /// Whether this response was served from cache
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_cache: Option<bool>,
}

pub struct SearchEngine {
    _config: Arc<Config>,
    _storage: StorageBackend,
    _tantivy_indexer: Arc<TantivyIndexer>,
    symbol_searcher: symbol::SymbolSearcher,
    #[cfg(feature = "semantic")]
    semantic_searcher: semantic::SemanticSearcher,
    cache: Arc<MultiTierCache>,
}

impl SearchEngine {
    pub async fn new(config: Arc<Config>, storage: StorageBackend) -> Result<Self> {
        // Create tantivy indexer for search operations (read-only)
        let index_path = config.cache_dir.join("tantivy_index");
        let tantivy_indexer = Arc::new(TantivyIndexer::new_read_only(&index_path).await?);

        let symbol_searcher =
            symbol::SymbolSearcher::new(config.clone(), storage.clone(), tantivy_indexer.clone())
                .await?;

        #[cfg(feature = "semantic")]
        let semantic_searcher =
            semantic::SemanticSearcher::new(config.clone(), storage.clone()).await?;

        // Initialize cache with default config
        let cache_config = CacheConfig::default();
        let cache = Arc::new(MultiTierCache::new(
            cache_config,
            Some(Arc::new(storage.clone())),
        ));

        Ok(Self {
            _config: config,
            _storage: storage,
            _tantivy_indexer: tantivy_indexer,
            symbol_searcher,
            #[cfg(feature = "semantic")]
            semantic_searcher,
            cache,
        })
    }

    pub async fn search(&self, query: SearchQuery) -> Result<SearchResponse> {
        let start = std::time::Instant::now();

        // Check cache first
        if let Some(mut cached_response) = self.cache.get(&query).await {
            cached_response.from_cache = Some(true);
            tracing::debug!("Serving search from cache for query: {}", query.query);
            return Ok(cached_response);
        }

        // Cache miss - perform actual search
        let results = match query.mode {
            SearchMode::Symbol => self.symbol_searcher.search(&query).await?,
            #[cfg(feature = "semantic")]
            SearchMode::Semantic => self.semantic_searcher.search(&query).await?,
            #[cfg(not(feature = "semantic"))]
            SearchMode::Semantic => {
                tracing::warn!("Semantic search requested but semantic feature is disabled");
                vec![]
            },
        };

        let total_matches = results.len();
        let results = results
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .collect();

        let response = SearchResponse {
            query: query.clone(),
            results,
            total_matches,
            search_time_ms: start.elapsed().as_millis() as u64,
            from_cache: Some(false),
        };

        // Store in cache for future queries
        if let Err(e) = self.cache.put(&query, response.clone()).await {
            tracing::warn!("Failed to cache search result: {}", e);
        }

        Ok(response)
    }

    /// Get cache metrics for monitoring
    pub fn cache_metrics(&self) -> Arc<crate::cache::CacheMetrics> {
        self.cache.metrics()
    }

    /// Clear the search cache
    pub async fn clear_cache(&self) {
        self.cache.clear().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexing::Indexer;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_symbol_search() {
        let temp_dir = tempdir().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        // Create test file
        fs::write(
            workspace.join("test.rs"),
            r#"
struct MyStruct {
    field: String,
}

impl MyStruct {
    fn new() -> Self {
        Self { field: String::new() }
    }
}

fn helper_function() {}
            "#,
        )
        .unwrap();

        let config = Arc::new(Config {
            workspace_roots: vec![workspace],
            cache_dir: temp_dir.path().join("cache"),
            ..Default::default()
        });

        let storage = StorageBackend::new(&config.cache_dir).await.unwrap();

        // Index files first
        {
            let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
            indexer.index_workspaces().await.unwrap();
            // Indexer is dropped here, releasing the writer lock
        }

        let search_engine = SearchEngine::new(config, storage).await.unwrap();

        // Test symbol search
        let query = SearchQuery {
            query: "MyStruct".to_string(),
            mode: SearchMode::Symbol,
            limit: 10,
            ..Default::default()
        };

        let response = search_engine.search(query).await.unwrap();
        assert!(response.total_matches > 0);
        assert!(
            response
                .results
                .iter()
                .any(|r| r.match_type == MatchType::Symbol)
        );
    }

    #[tokio::test]
    async fn test_search_with_filters() {
        let temp_dir = tempdir().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        // Create test files
        fs::write(workspace.join("main.rs"), "fn main() {}").unwrap();
        fs::write(workspace.join("test.py"), "def main(): pass").unwrap();
        fs::write(workspace.join("index.js"), "function main() {}").unwrap();

        let config = Arc::new(Config {
            workspace_roots: vec![workspace],
            cache_dir: temp_dir.path().join("cache"),
            ..Default::default()
        });

        let storage = StorageBackend::new(&config.cache_dir).await.unwrap();

        // Index files first
        {
            let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
            indexer.index_workspaces().await.unwrap();
            // Indexer is dropped here, releasing the writer lock
        }

        let search_engine = SearchEngine::new(config, storage).await.unwrap();

        // Test search with file pattern filter using symbol mode
        let query = SearchQuery {
            query: "main".to_string(),
            mode: SearchMode::Symbol,
            file_patterns: Some(vec!["*.rs".to_string()]),
            limit: 10,
            ..Default::default()
        };

        let response = search_engine.search(query).await.unwrap();
        // Symbol search looks for definitions, so we expect to find the main function
        assert!(response.total_matches >= 1);
        assert!(
            response
                .results
                .iter()
                .all(|r| r.file_path.extension().is_some_and(|e| e == "rs"))
        );
    }
}
