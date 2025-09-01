use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tantivy::query::QueryParser;
use tracing::debug;

use super::{MatchType, SearchQuery, SearchResult};
use crate::{Config, indexing::tantivy_indexer::TantivyIndexer, storage::StorageBackend};

#[derive(Clone)]
pub struct LiteralSearcher {
    config: Arc<Config>,
    storage: StorageBackend,
    tantivy_indexer: Arc<TantivyIndexer>,
}

impl LiteralSearcher {
    pub async fn new(
        config: Arc<Config>,
        storage: StorageBackend,
        tantivy_indexer: Arc<TantivyIndexer>,
    ) -> Result<Self> {
        Ok(Self {
            config,
            storage,
            tantivy_indexer,
        })
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        debug!("Performing literal search for: {}", query.query);

        // Build Tantivy query
        let query_parser = QueryParser::for_index(
            self.tantivy_indexer.get_searcher().index(),
            vec![
                self.tantivy_indexer.get_content_field(),
                self.tantivy_indexer.get_symbols_field(),
            ],
        );

        let tantivy_query = query_parser.parse_query(&query.query)?;

        // Search documents - fetch extra to account for multiple matches per document
        // and to ensure we have enough results after applying offset
        let fetch_limit = (query.limit + query.offset) * 10; // Fetch 10x to ensure we have enough
        let docs = self
            .tantivy_indexer
            .search_documents(tantivy_query.as_ref(), fetch_limit)
            .await?;

        // Convert to SearchResult with line numbers and context
        let mut results = Vec::new();

        for doc in docs {
            // Apply repository filter if specified
            if let Some(repos) = &query.repositories
                && !repos.contains(&doc.repository)
            {
                continue;
            }

            // Apply file pattern filter if specified
            if let Some(patterns) = &query.file_patterns {
                let file_name = doc.path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                let matches_pattern = patterns.iter().any(|pattern| {
                    if pattern.contains('*') {
                        // Simple glob matching
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

            // Find matches in content and create results with line context
            let search_results = self.find_matches_in_content(
                &doc.path,
                &doc.repository,
                &doc.content,
                &query.query,
                doc.score,
            )?;

            results.extend(search_results);
        }

        // Apply offset and limit to the final results
        let start = query.offset.min(results.len());
        let end = (start + query.limit).min(results.len());
        Ok(results[start..end].to_vec())
    }

    fn find_matches_in_content(
        &self,
        file_path: &Path,
        repository: &str,
        content: &str,
        search_term: &str,
        score: f32,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let search_term_lower = search_term.to_lowercase();

        for (line_idx, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();

            // Find all occurrences in this line
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&search_term_lower) {
                let column = start + pos;

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
                    score,
                    match_type: MatchType::Exact,
                });

                start = column + search_term.len();
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
#[path = "literal_test.rs"]
mod tests;
