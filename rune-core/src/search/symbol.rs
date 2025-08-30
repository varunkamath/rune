use std::sync::Arc;
use anyhow::Result;

use crate::{Config, storage::StorageBackend};
use super::{SearchQuery, SearchResult};

#[derive(Clone)]
pub struct SymbolSearcher {
    config: Arc<Config>,
    storage: StorageBackend,
}

impl SymbolSearcher {
    pub async fn new(config: Arc<Config>, storage: StorageBackend) -> Result<Self> {
        Ok(Self { config, storage })
    }
    
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        // TODO: Implement tree-sitter based symbol search
        Ok(vec![])
    }
}