use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::{SearchQuery, SearchResult};
use crate::{Config, embedding::EmbeddingPipeline, storage::StorageBackend};

#[derive(Clone)]
pub struct SemanticSearcher {
    config: Arc<Config>,
    storage: StorageBackend,
    pipeline: Option<Arc<EmbeddingPipeline>>,
}

impl SemanticSearcher {
    pub async fn new(config: Arc<Config>, storage: StorageBackend) -> Result<Self> {
        // Try to initialize the embedding pipeline
        let pipeline = match EmbeddingPipeline::new(config.clone()).await {
            Ok(p) => {
                info!("Semantic search initialized successfully");
                Some(Arc::new(p))
            },
            Err(e) => {
                warn!(
                    "Failed to initialize semantic search: {}. Feature will be disabled.",
                    e
                );
                None
            },
        };

        Ok(Self {
            config,
            storage,
            pipeline,
        })
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        if let Some(ref pipeline) = self.pipeline {
            if !pipeline.is_available() {
                debug!("Semantic search pipeline not available");
                return Ok(vec![]);
            }

            debug!("Performing semantic search for: {}", query.query);

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

            debug!("Found {} semantic search results", results.len());
            Ok(results)
        } else {
            debug!("Semantic search not available");
            Ok(vec![])
        }
    }

    /// Process files for semantic indexing
    pub async fn index_file(&self, file_path: &str, content: &str) -> Result<()> {
        if let Some(ref pipeline) = self.pipeline
            && pipeline.is_available()
        {
            debug!("Indexing file for semantic search: {}", file_path);
            pipeline.process_file(file_path, content).await?;
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
