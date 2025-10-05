use anyhow::Result;
use dashmap::DashMap;
use regex::Regex;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tracing::debug;

use super::{MatchType, SearchQuery, SearchResult};
use crate::{Config, indexing::tantivy_indexer::TantivyIndexer, storage::StorageBackend};

pub struct RegexSearcher {
    config: Arc<Config>,
    storage: StorageBackend,
    tantivy_indexer: Arc<TantivyIndexer>,
    regex_cache: DashMap<String, Regex>,
}

impl RegexSearcher {
    pub async fn new(
        config: Arc<Config>,
        storage: StorageBackend,
        tantivy_indexer: Arc<TantivyIndexer>,
    ) -> Result<Self> {
        Ok(Self {
            config,
            storage,
            tantivy_indexer,
            regex_cache: DashMap::new(),
        })
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        debug!("Performing regex search for: {}", query.query);

        // Compile regex with caching
        let regex = if let Some(cached) = self.regex_cache.get(&query.query) {
            cached.clone()
        } else {
            let regex = Regex::new(&query.query)?;
            self.regex_cache.insert(query.query.clone(), regex.clone());
            regex
        };

        // Get all indexed files from tantivy (we need to search through actual content)
        // For regex search, we can't use Tantivy's query parser directly
        // Instead, we'll get all files and search through them
        let mut results = Vec::new();

        // Get file list from storage
        let files = self.storage.list_files().await?;

        // Scan all files to find matches (pagination handled centrally in SearchEngine::search())
        for file_path in files.iter() {
            // Apply repository filter if specified
            if let Some(repos) = &query.repositories {
                let repo = file_path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                if !repos.contains(&repo.to_string()) {
                    continue;
                }
            }

            // Apply file pattern filter if specified
            if let Some(patterns) = &query.file_patterns {
                let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                let matches_pattern = patterns.iter().any(|pattern| {
                    if pattern.contains('*') {
                        let pattern = pattern.replace("*", "");
                        file_name.contains(&pattern)
                    } else {
                        file_name == pattern
                    }
                });

                if !matches_pattern {
                    continue;
                }
            }

            // Read file content
            if let Ok(content) = fs::read_to_string(&file_path).await {
                let repo = file_path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                // Find matches using regex
                let search_results = self.find_regex_matches(file_path, repo, &content, &regex)?;

                results.extend(search_results);
            }
        }

        Ok(results)
    }

    fn find_regex_matches(
        &self,
        file_path: &Path,
        repository: &str,
        content: &str,
        regex: &Regex,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            // Find all regex matches in this line
            for mat in regex.find_iter(line) {
                let column = mat.start();

                // Get context lines (3 before, 3 after)
                let context_before: Vec<String> = lines
                    .iter()
                    .skip(line_idx.saturating_sub(3))
                    .take(line_idx.min(3))
                    .map(|s| s.to_string())
                    .collect();

                let context_after: Vec<String> = lines
                    .iter()
                    .skip(line_idx + 1)
                    .take(3)
                    .map(|s| s.to_string())
                    .collect();

                results.push(SearchResult {
                    file_path: file_path.to_path_buf(),
                    repository: repository.to_string(),
                    line_number: line_idx + 1, // 1-indexed
                    column,
                    content: line.to_string(),
                    context_before,
                    context_after,
                    score: 1.0, // Regex matches don't have scores
                    match_type: MatchType::Exact,
                });
            }
        }

        Ok(results)
    }
}

// Clone implementation for RegexSearcher
impl Clone for RegexSearcher {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: self.storage.clone(),
            tantivy_indexer: self.tantivy_indexer.clone(),
            regex_cache: DashMap::new(), // Don't clone the cache
        }
    }
}

#[cfg(test)]
#[path = "regex_test.rs"]
mod tests;
