use anyhow::Result;
use std::sync::Arc;

use super::{SearchQuery, SearchResult};
use crate::{Config, storage::StorageBackend};

#[derive(Clone)]
pub struct SymbolSearcher {
    config: Arc<Config>,
    storage: StorageBackend,
}

impl SymbolSearcher {
    pub async fn new(config: Arc<Config>, storage: StorageBackend) -> Result<Self> {
        Ok(Self { config, storage })
    }

    pub async fn search(&self, _query: &SearchQuery) -> Result<Vec<SearchResult>> {
        // TODO: Implement tree-sitter based symbol search
        Ok(vec![])
    }
}
