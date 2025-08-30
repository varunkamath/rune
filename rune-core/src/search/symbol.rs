use anyhow::Result;
use std::sync::Arc;
use tantivy::query::QueryParser;
use tracing::debug;

use super::{MatchType, SearchQuery, SearchResult};
use crate::{Config, indexing::tantivy_indexer::TantivyIndexer, storage::StorageBackend};

#[derive(Clone)]
pub struct SymbolSearcher {
    config: Arc<Config>,
    storage: StorageBackend,
    tantivy_indexer: Arc<TantivyIndexer>,
}

impl SymbolSearcher {
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
        debug!("Performing symbol search for: {}", query.query);

        // Build Tantivy query specifically for symbols field
        let query_parser = QueryParser::for_index(
            self.tantivy_indexer.get_searcher().index(),
            vec![self.tantivy_indexer.get_symbols_field()],
        );

        // The query should match symbol names or types
        // For now, just search for the symbol name in the symbols field
        let search_query = query.query.clone();

        let tantivy_query = query_parser.parse_query(&search_query)?;

        // Search documents
        let docs = self
            .tantivy_indexer
            .search_documents(tantivy_query.as_ref(), query.limit + query.offset)
            .await?;

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

            // Parse symbols from the content to find exact matches
            let symbol_matches = self.find_symbol_matches(
                &doc.path,
                &doc.repository,
                &doc.content,
                &query.query,
                doc.score,
            )?;

            results.extend(symbol_matches);
        }

        Ok(results)
    }

    fn find_symbol_matches(
        &self,
        file_path: &std::path::Path,
        repository: &str,
        content: &str,
        symbol_query: &str,
        score: f32,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        // Simple heuristic: look for the symbol name in function/class/struct definitions
        // This is a simplified approach - in production, we'd re-parse with tree-sitter
        let symbol_name = symbol_query;

        for (line_idx, line) in lines.iter().enumerate() {
            // Check if this line likely contains a symbol definition
            let line_lower = line.to_lowercase();
            let symbol_lower = symbol_name.to_lowercase();

            // Look for common patterns that indicate symbol definitions
            let is_symbol_def = (line_lower.contains("fn ") && line_lower.contains(&symbol_lower))
                || (line_lower.contains("function ") && line_lower.contains(&symbol_lower))
                || (line_lower.contains("def ") && line_lower.contains(&symbol_lower))
                || (line_lower.contains("class ") && line_lower.contains(&symbol_lower))
                || (line_lower.contains("struct ") && line_lower.contains(&symbol_lower))
                || (line_lower.contains("interface ") && line_lower.contains(&symbol_lower))
                || (line_lower.contains("trait ") && line_lower.contains(&symbol_lower))
                || (line_lower.contains("impl ") && line_lower.contains(&symbol_lower))
                || (line_lower.contains("type ") && line_lower.contains(&symbol_lower))
                || (line_lower.contains("enum ") && line_lower.contains(&symbol_lower));

            if is_symbol_def {
                // Find the column where the symbol name appears
                let column = line_lower.find(&symbol_lower).unwrap_or(0);

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
                    match_type: MatchType::Symbol,
                });
            }
        }

        Ok(results)
    }
}
