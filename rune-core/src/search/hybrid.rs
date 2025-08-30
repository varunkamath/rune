use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

use super::literal::LiteralSearcher;
use super::symbol::SymbolSearcher;
use super::{SearchQuery, SearchResult};

#[cfg(feature = "embeddings")]
use super::semantic::SemanticSearcher;

pub struct HybridSearcher {
    literal_searcher: LiteralSearcher,
    symbol_searcher: SymbolSearcher,
    #[cfg(feature = "embeddings")]
    semantic_searcher: SemanticSearcher,
}

impl HybridSearcher {
    pub fn new(
        literal_searcher: LiteralSearcher,
        symbol_searcher: SymbolSearcher,
        #[cfg(feature = "embeddings")] semantic_searcher: SemanticSearcher,
    ) -> Self {
        Self {
            literal_searcher,
            symbol_searcher,
            #[cfg(feature = "embeddings")]
            semantic_searcher,
        }
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        debug!("Performing hybrid search for: {}", query.query);

        // Get results from all searchers
        let literal_results = self.literal_searcher.search(query).await?;
        let symbol_results = self.symbol_searcher.search(query).await?;

        #[cfg(feature = "embeddings")]
        let semantic_results = self.semantic_searcher.search(query).await?;

        // Implement Reciprocal Rank Fusion (RRF)
        // RRF score = sum(1 / (k + rank_i)) where k is a constant (typically 60)
        let k = 60.0;

        // Create a map to store RRF scores for each unique result
        // Key is (file_path, line_number)
        let mut rrf_scores: HashMap<(PathBuf, usize), (SearchResult, f32)> = HashMap::new();

        // Process literal results
        for (rank, result) in literal_results.into_iter().enumerate() {
            let key = (result.file_path.clone(), result.line_number);
            let rrf_score = 1.0 / (k + rank as f32 + 1.0);

            rrf_scores
                .entry(key)
                .and_modify(|(_, score)| *score += rrf_score)
                .or_insert((result, rrf_score));
        }

        // Process symbol results
        for (rank, result) in symbol_results.into_iter().enumerate() {
            let key = (result.file_path.clone(), result.line_number);
            let rrf_score = 1.0 / (k + rank as f32 + 1.0);

            rrf_scores
                .entry(key)
                .and_modify(|(_, score)| *score += rrf_score)
                .or_insert((result, rrf_score));
        }

        // Process semantic results if available
        #[cfg(feature = "embeddings")]
        for (rank, result) in semantic_results.into_iter().enumerate() {
            let key = (result.file_path.clone(), result.line_number);
            let rrf_score = 1.0 / (k + rank as f32 + 1.0);

            rrf_scores
                .entry(key)
                .and_modify(|(_, score)| *score += rrf_score)
                .or_insert((result, rrf_score));
        }

        // Sort by RRF score and extract results
        let mut final_results: Vec<(SearchResult, f32)> = rrf_scores.into_values().collect();
        final_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Update scores in results and return
        let results: Vec<SearchResult> = final_results
            .into_iter()
            .map(|(mut result, rrf_score)| {
                result.score = rrf_score;
                result
            })
            .collect();

        debug!("Hybrid search returned {} results", results.len());
        Ok(results)
    }
}
