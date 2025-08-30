use anyhow::Result;

use super::{SearchQuery, SearchResult};
use super::literal::LiteralSearcher;

#[cfg(feature = "embeddings")]
use super::semantic::SemanticSearcher;

pub struct HybridSearcher {
    literal_searcher: LiteralSearcher,
    #[cfg(feature = "embeddings")]
    semantic_searcher: SemanticSearcher,
}

impl HybridSearcher {
    pub fn new(
        literal_searcher: LiteralSearcher,
        #[cfg(feature = "embeddings")]
        semantic_searcher: SemanticSearcher,
    ) -> Self {
        Self {
            literal_searcher,
            #[cfg(feature = "embeddings")]
            semantic_searcher,
        }
    }
    
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        // Get results from both searchers
        let literal_results = self.literal_searcher.search(query).await?;
        
        #[cfg(feature = "embeddings")]
        let semantic_results = self.semantic_searcher.search(query).await?;
        
        // TODO: Implement Reciprocal Rank Fusion (RRF)
        // For now, just return literal results
        Ok(literal_results)
    }
}