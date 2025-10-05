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
    _config: Arc<Config>,     // Kept for potential future use
    _storage: StorageBackend, // Kept for potential future use
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
            _config: config,
            _storage: storage,
            tantivy_indexer,
            fuzzy_matcher,
        })
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        debug!("Performing literal search for: {}", query.query);

        // Log if multi-word query
        let word_count = query.query.split_whitespace().count();
        if word_count > 1 {
            debug!("Multi-word query detected with {} words", word_count);
        }

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

        debug!(
            "Tantivy returned {} documents for literal search",
            docs.len()
        );

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

        debug!("Literal search found {} total line matches", results.len());

        // Note: Pagination is handled centrally in SearchEngine::search()
        // Return all results without applying offset/limit here to avoid double pagination
        Ok(results)
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

        // Split the search term into individual words for multi-term queries
        let search_words: Vec<String> = search_term
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        // If it's a single word, use the original exact matching logic
        if search_words.len() == 1 {
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
        } else {
            // Multi-word search: find lines containing ANY of the words
            for (line_idx, line) in lines.iter().enumerate() {
                let line_lower = line.to_lowercase();
                let mut line_has_match = false;
                let mut first_match_column = 0;
                let mut match_count = 0;

                // Check if this line contains any of the search words
                for word in &search_words {
                    if let Some(pos) = line_lower.find(word) {
                        if !line_has_match {
                            first_match_column = pos;
                            line_has_match = true;
                        }
                        match_count += 1;
                    }
                }

                // If this line contains at least one search word, add it to results
                if line_has_match {
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

                    // Boost score based on how many terms matched
                    let boosted_score = score * (1.0 + (match_count as f32 - 1.0) * 0.5);

                    results.push(SearchResult {
                        file_path: file_path.to_path_buf(),
                        repository: repository.to_string(),
                        line_number: line_idx + 1, // 1-indexed
                        column: first_match_column,
                        content: line.to_string(),
                        context_before,
                        context_after,
                        score: boosted_score,
                        match_type: MatchType::Exact,
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
