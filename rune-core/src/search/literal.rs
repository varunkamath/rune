use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tantivy::Term;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query, QueryParser};
use tracing::debug;

use super::{
    MatchType, SearchQuery, SearchResult,
    fuzzy_match::{FuzzyConfig, FuzzyMatcher},
};
use crate::{Config, indexing::tantivy_indexer::TantivyIndexer, storage::StorageBackend};

#[derive(Clone)]
pub struct LiteralSearcher {
    config: Arc<Config>,
    storage: StorageBackend,
    tantivy_indexer: Arc<TantivyIndexer>,
    fuzzy_matcher: FuzzyMatcher,
}

impl LiteralSearcher {
    pub async fn new(
        config: Arc<Config>,
        storage: StorageBackend,
        tantivy_indexer: Arc<TantivyIndexer>,
    ) -> Result<Self> {
        let fuzzy_matcher = FuzzyMatcher::with_config(FuzzyConfig::default());
        Ok(Self {
            config,
            storage,
            tantivy_indexer,
            fuzzy_matcher,
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

        // Parse the standard query
        let exact_query = query_parser.parse_query(&query.query)?;

        // If fuzzy matching is enabled, create a combined query with fuzzy terms
        let tantivy_query: Box<dyn Query> = if self.fuzzy_matcher.is_enabled() {
            // Create a boolean query that combines exact and fuzzy matches
            let mut subqueries = vec![(Occur::Should, exact_query)];

            // Add fuzzy queries for each term in the query
            let query_words: Vec<&str> = query.query.split_whitespace().collect();
            for word in query_words {
                // Create fuzzy term queries for both content and symbols fields
                let content_term =
                    Term::from_field_text(self.tantivy_indexer.get_content_field(), word);
                let fuzzy_content = FuzzyTermQuery::new(content_term, 2, true); // Max edit distance 2

                let symbols_term =
                    Term::from_field_text(self.tantivy_indexer.get_symbols_field(), word);
                let fuzzy_symbols = FuzzyTermQuery::new(symbols_term, 2, true); // Max edit distance 2

                // Add fuzzy queries as optional (SHOULD) clauses
                subqueries.push((Occur::Should, Box::new(fuzzy_content) as Box<dyn Query>));
                subqueries.push((Occur::Should, Box::new(fuzzy_symbols) as Box<dyn Query>));
            }

            Box::new(BooleanQuery::new(subqueries))
        } else {
            exact_query
        };

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

            // First, find exact matches
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

            // If fuzzy matching is enabled and we haven't found exact matches on this line,
            // look for fuzzy matches
            if self.fuzzy_matcher.is_enabled() && !line_lower.contains(&search_term_lower) {
                let fuzzy_matches = self.fuzzy_matcher.find_fuzzy_matches(search_term, line);

                for fuzzy_match in fuzzy_matches {
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

                    // Adjust score based on fuzzy match similarity
                    let fuzzy_score = score * fuzzy_match.similarity as f32;

                    results.push(SearchResult {
                        file_path: file_path.to_path_buf(),
                        repository: repository.to_string(),
                        line_number: line_idx + 1, // 1-indexed
                        column: fuzzy_match.position,
                        content: line.to_string(),
                        context_before,
                        context_after,
                        score: fuzzy_score,
                        match_type: MatchType::Fuzzy,
                    });
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
#[path = "literal_test.rs"]
mod tests;
