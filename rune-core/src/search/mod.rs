pub mod literal;
pub mod regex;
pub mod semantic;
pub mod symbol;
pub mod hybrid;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{Config, storage::StorageBackend};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchMode {
    Literal,
    Regex,
    Symbol,
    Semantic,
    Hybrid,
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
            mode: SearchMode::Hybrid,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    Exact,
    Fuzzy,
    Semantic,
    Symbol,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: SearchQuery,
    pub results: Vec<SearchResult>,
    pub total_matches: usize,
    pub search_time_ms: u64,
}

pub struct SearchEngine {
    config: Arc<Config>,
    storage: StorageBackend,
    literal_searcher: literal::LiteralSearcher,
    regex_searcher: regex::RegexSearcher,
    symbol_searcher: symbol::SymbolSearcher,
    #[cfg(feature = "embeddings")]
    semantic_searcher: semantic::SemanticSearcher,
    hybrid_searcher: hybrid::HybridSearcher,
}

impl SearchEngine {
    pub async fn new(config: Arc<Config>, storage: StorageBackend) -> Result<Self> {
        let literal_searcher = literal::LiteralSearcher::new(config.clone(), storage.clone()).await?;
        let regex_searcher = regex::RegexSearcher::new(config.clone())?;
        let symbol_searcher = symbol::SymbolSearcher::new(config.clone(), storage.clone()).await?;
        
        #[cfg(feature = "embeddings")]
        let semantic_searcher = semantic::SemanticSearcher::new(config.clone(), storage.clone()).await?;
        
        let hybrid_searcher = hybrid::HybridSearcher::new(
            literal_searcher.clone(),
            #[cfg(feature = "embeddings")]
            semantic_searcher.clone(),
        );
        
        Ok(Self {
            config,
            storage,
            literal_searcher,
            regex_searcher,
            symbol_searcher,
            #[cfg(feature = "embeddings")]
            semantic_searcher,
            hybrid_searcher,
        })
    }
    
    pub async fn search(&self, query: SearchQuery) -> Result<SearchResponse> {
        let start = std::time::Instant::now();
        
        let results = match query.mode {
            SearchMode::Literal => self.literal_searcher.search(&query).await?,
            SearchMode::Regex => self.regex_searcher.search(&query).await?,
            SearchMode::Symbol => self.symbol_searcher.search(&query).await?,
            #[cfg(feature = "embeddings")]
            SearchMode::Semantic => self.semantic_searcher.search(&query).await?,
            #[cfg(not(feature = "embeddings"))]
            SearchMode::Semantic => {
                tracing::warn!("Semantic search requested but embeddings feature is disabled");
                vec![]
            },
            SearchMode::Hybrid => self.hybrid_searcher.search(&query).await?,
        };
        
        let total_matches = results.len();
        let results = results
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .collect();
        
        Ok(SearchResponse {
            query,
            results,
            total_matches,
            search_time_ms: start.elapsed().as_millis() as u64,
        })
    }
    
    pub async fn reindex(&self) -> Result<()> {
        // Trigger reindexing
        Ok(())
    }
}