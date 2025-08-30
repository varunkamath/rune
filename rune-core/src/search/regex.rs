use anyhow::Result;
use regex::Regex;
use std::sync::Arc;

use super::{SearchQuery, SearchResult};
use crate::Config;

pub struct RegexSearcher {
    config: Arc<Config>,
}

impl RegexSearcher {
    pub fn new(config: Arc<Config>) -> Result<Self> {
        Ok(Self { config })
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        // Compile regex
        let _regex = Regex::new(&query.query)?;

        // TODO: Implement regex search across files
        Ok(vec![])
    }
}
