use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::{SearchQuery, SearchResult};
use crate::{Config, embedding::EmbeddingPipeline, storage::StorageBackend};

#[derive(Clone)]
pub struct SemanticSearcher {
    _config: Arc<Config>,     // Kept for potential future use
    _storage: StorageBackend, // Kept for potential future use
    pipeline: Option<Arc<EmbeddingPipeline>>,
}

impl SemanticSearcher {
    pub async fn new(config: Arc<Config>, storage: StorageBackend) -> Result<Self> {
        // Try to initialize the embedding pipeline
        let pipeline = match EmbeddingPipeline::new(config.clone()).await {
            Ok(p) => {
                if p.is_available() {
                    info!(
                        "[SEMANTIC] Semantic search initialized successfully with Qdrant backend"
                    );
                    Some(Arc::new(p))
                } else {
                    warn!(
                        "[SEMANTIC] Embedding pipeline created but Qdrant is not available. Semantic search will be disabled."
                    );
                    None
                }
            },
            Err(e) => {
                warn!(
                    "[SEMANTIC] Failed to initialize semantic search: {}. Feature will be disabled.",
                    e
                );
                None
            },
        };

        Ok(Self {
            _config: config,
            _storage: storage,
            pipeline,
        })
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        if let Some(ref pipeline) = self.pipeline {
            if !pipeline.is_available() {
                debug!("[SEMANTIC] Pipeline exists but is not available (Qdrant disconnected)");
                return Ok(vec![]);
            }

            debug!("[SEMANTIC] Performing semantic search for: {}", query.query);

            // Perform semantic search
            let semantic_results = pipeline.search(&query.query, query.limit).await?;

            // Convert to SearchResult format
            let mut results = Vec::new();
            for result in semantic_results.iter() {
                // Apply repository and file pattern filters if specified
                if let Some(ref repos) = query.repositories {
                    let repo = self.extract_repo_from_path(&result.file_path);
                    if !repos.iter().any(|r| r == &repo) {
                        continue;
                    }
                }

                if let Some(ref patterns) = query.file_patterns
                    && !self.matches_patterns(&result.file_path, patterns)
                {
                    continue;
                }

                results.push(SearchResult {
                    file_path: PathBuf::from(&result.file_path),
                    repository: self.extract_repo_from_path(&result.file_path),
                    line_number: result.start_line,
                    column: 0,
                    content: result.content.clone(),
                    context_before: Vec::new(),
                    context_after: Vec::new(),
                    score: result.score,
                    match_type: super::MatchType::Semantic,
                });

                if results.len() >= query.limit {
                    break;
                }
            }

            debug!("[SEMANTIC] Found {} results after filtering", results.len());
            Ok(results)
        } else {
            debug!("[SEMANTIC] Search skipped - pipeline not initialized");
            Ok(vec![])
        }
    }

    /// Process files for semantic indexing
    pub async fn index_file(&self, file_path: &str, content: &str) -> Result<()> {
        info!("[SEMANTIC] Attempting to index file: {}", file_path);

        if let Some(ref pipeline) = self.pipeline {
            if pipeline.is_available() {
                info!(
                    "[SEMANTIC] Pipeline available, processing file: {}",
                    file_path
                );
                pipeline.process_file(file_path, content).await?;
                info!("[SEMANTIC] Successfully indexed file: {}", file_path);
            } else {
                warn!("[SEMANTIC] Pipeline not available for file: {}", file_path);
            }
        } else {
            warn!("[SEMANTIC] No pipeline configured for semantic search");
        }

        Ok(())
    }

    /// Clear semantic index
    pub async fn clear_index(&self) -> Result<()> {
        if let Some(ref pipeline) = self.pipeline {
            pipeline.clear().await?;
        }
        Ok(())
    }

    /// Check if semantic search is available
    pub fn is_available(&self) -> bool {
        self.pipeline.as_ref().is_some_and(|p| p.is_available())
    }

    // Helper methods

    fn extract_repo_from_path(&self, path: &str) -> String {
        // Extract repository name from path
        // Assumes format: repo_name/path/to/file
        path.split('/').next().unwrap_or("unknown").to_string()
    }

    fn matches_patterns(&self, path: &str, patterns: &[String]) -> bool {
        for pattern in patterns {
            if pattern.contains('*') {
                // Simple glob matching
                let pattern = pattern.replace("**", ".*").replace('*', "[^/]*");
                if let Ok(re) = regex::Regex::new(&pattern)
                    && re.is_match(path)
                {
                    return true;
                }
            } else if path.contains(pattern) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageBackend;
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
            languages: vec!["rust".to_string()],
            file_watch_debounce_ms: 500,
        })
    }

    #[tokio::test]
    async fn test_semantic_searcher_initialization_without_qdrant() {
        // When Qdrant is not available, the searcher should initialize but pipeline should be None
        let config = create_test_config();
        let storage = StorageBackend::new(&config.cache_dir).await.unwrap();

        // This test will pass regardless of Qdrant availability
        let searcher = SemanticSearcher::new(config, storage).await.unwrap();

        // If Qdrant is not running, is_available should return false
        // If it is running, it should return true
        let _ = searcher.is_available(); // Don't assert, just check it doesn't panic
    }

    #[tokio::test]
    #[ignore = "This test requires Qdrant to not be running"]
    async fn test_search_with_no_pipeline_returns_empty() {
        // Explicitly disable semantic search and set a bad Qdrant URL
        unsafe {
            std::env::set_var("RUNE_ENABLE_SEMANTIC", "false");
            std::env::set_var("QDRANT_URL", "http://127.0.0.1:99999"); // Non-existent port
        }

        let config = Arc::new(Config {
            workspace_roots: vec![],
            workspace_dir: String::new(),
            cache_dir: tempdir().unwrap().path().to_path_buf(),
            max_file_size: 10 * 1024 * 1024,
            indexing_threads: 1,
            enable_semantic: false, // Disable semantic to ensure no pipeline
            languages: vec![],
            file_watch_debounce_ms: 500,
        });

        let storage = StorageBackend::new(&config.cache_dir).await.unwrap();
        let searcher = SemanticSearcher::new(config, storage).await.unwrap();

        // Clean up env vars before assertions that might fail
        unsafe {
            std::env::remove_var("RUNE_ENABLE_SEMANTIC");
            std::env::remove_var("QDRANT_URL");
        }

        let query = SearchQuery {
            query: "test query".to_string(),
            mode: super::super::SearchMode::Semantic,
            repositories: None,
            file_patterns: None,
            limit: 10,
            offset: 0,
        };

        let results = searcher.search(&query).await.unwrap();
        assert_eq!(
            results.len(),
            0,
            "Should return empty results when semantic is disabled"
        );
        assert!(
            !searcher.is_available(),
            "Searcher should not be available when semantic is disabled"
        );
    }

    #[tokio::test]
    async fn test_extract_repo_from_path() {
        let config = create_test_config();
        let storage = StorageBackend::new(&config.cache_dir).await.unwrap();

        // We need to test synchronously, so we'll test the helper method directly
        // by creating a minimal searcher
        let searcher = SemanticSearcher {
            _config: config,
            _storage: storage,
            pipeline: None,
        };

        assert_eq!(searcher.extract_repo_from_path("repo/path/file.rs"), "repo");
        assert_eq!(searcher.extract_repo_from_path("single.rs"), "single.rs");
        assert_eq!(
            searcher.extract_repo_from_path("deep/nested/path/file.rs"),
            "deep"
        );
    }

    #[tokio::test]
    async fn test_matches_patterns() {
        let config = create_test_config();
        let storage = StorageBackend::new(&config.cache_dir).await.unwrap();
        let searcher = SemanticSearcher {
            _config: config,
            _storage: storage,
            pipeline: None,
        };

        // Test exact match
        assert!(searcher.matches_patterns("test.rs", &["test.rs".to_string()]));

        // Test glob patterns
        assert!(searcher.matches_patterns("src/main.rs", &["*.rs".to_string()]));
        assert!(searcher.matches_patterns("src/lib.rs", &["src/*.rs".to_string()]));
        assert!(searcher.matches_patterns("deep/nested/file.rs", &["**/*.rs".to_string()]));

        // Test partial match
        assert!(searcher.matches_patterns("src/main.rs", &["main".to_string()]));

        // Test no match
        assert!(!searcher.matches_patterns("test.py", &["*.rs".to_string()]));
        assert!(!searcher.matches_patterns("test.rs", &["*.py".to_string()]));
    }

    #[tokio::test]
    async fn test_search_with_filters() {
        let config = create_test_config();
        let storage = StorageBackend::new(&config.cache_dir).await.unwrap();
        let searcher = SemanticSearcher::new(config, storage).await.unwrap();

        // Create a query with filters
        let query = SearchQuery {
            query: "test".to_string(),
            mode: super::super::SearchMode::Semantic,
            repositories: Some(vec!["test_repo".to_string()]),
            file_patterns: Some(vec!["*.rs".to_string()]),
            limit: 5,
            offset: 0,
        };

        // This should not panic even without pipeline
        let results = searcher.search(&query).await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_clear_index_without_pipeline() {
        let config = create_test_config();
        let storage = StorageBackend::new(&config.cache_dir).await.unwrap();
        let searcher = SemanticSearcher::new(config, storage).await.unwrap();

        // Should not panic even without pipeline
        searcher.clear_index().await.unwrap();
    }

    #[tokio::test]
    async fn test_index_file_without_pipeline() {
        let config = Arc::new(Config {
            workspace_roots: vec![],
            workspace_dir: String::new(),
            cache_dir: tempdir().unwrap().path().to_path_buf(),
            max_file_size: 10 * 1024 * 1024,
            indexing_threads: 1,
            enable_semantic: false,
            languages: vec![],
            file_watch_debounce_ms: 500,
        });

        let storage = StorageBackend::new(&config.cache_dir).await.unwrap();
        let searcher = SemanticSearcher::new(config, storage).await.unwrap();

        // Should handle gracefully without pipeline
        searcher
            .index_file("test.rs", "fn main() {}")
            .await
            .unwrap();
    }
}
