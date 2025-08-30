use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use tantivy::{
    Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, doc,
    schema::{FAST, Field, STORED, STRING, Schema, TEXT, Value},
};
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::language_detector::LanguageDetector;
use super::symbol_extractor::SymbolExtractor;

pub struct TantivyIndexer {
    index: Index,
    schema: Schema,
    writer: Option<Arc<RwLock<IndexWriter>>>,
    reader: IndexReader,

    // Field handles
    path_field: Field,
    content_field: Field,
    language_field: Field,
    symbols_field: Field,
    line_numbers_field: Field,
    repository_field: Field,
}

impl TantivyIndexer {
    pub async fn new(index_path: &Path) -> Result<Self> {
        Self::new_with_writer(index_path, true).await
    }

    pub async fn new_read_only(index_path: &Path) -> Result<Self> {
        Self::new_with_writer(index_path, false).await
    }

    async fn new_with_writer(index_path: &Path, create_writer: bool) -> Result<Self> {
        // Create index directory
        tokio::fs::create_dir_all(index_path).await?;

        // Build schema
        let mut schema_builder = Schema::builder();

        let path_field = schema_builder.add_text_field("path", STRING | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let language_field = schema_builder.add_text_field("language", STRING | STORED | FAST);
        let symbols_field = schema_builder.add_text_field("symbols", TEXT | STORED);
        let line_numbers_field = schema_builder.add_text_field("line_numbers", STORED);
        let repository_field = schema_builder.add_text_field("repository", STRING | STORED | FAST);

        let schema = schema_builder.build();

        // Open or create index
        let index = if index_path.join("meta.json").exists() {
            Index::open_in_dir(index_path)?
        } else {
            Index::create_in_dir(index_path, schema.clone())?
        };

        // Create writer with 100MB heap if requested
        let writer = if create_writer {
            Some(Arc::new(RwLock::new(index.writer(100_000_000)?)))
        } else {
            None
        };

        // Create reader with automatic reloading
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            schema,
            writer,
            reader,
            path_field,
            content_field,
            language_field,
            symbols_field,
            line_numbers_field,
            repository_field,
        })
    }

    pub async fn index_file(
        &self,
        file_path: &Path,
        repository: &str,
        content: &str,
    ) -> Result<()> {
        // Detect language
        let language = LanguageDetector::detect(file_path, Some(content));

        // Extract symbols if supported
        let symbols = if language.supports_tree_sitter() {
            let extractor = SymbolExtractor::new();
            extractor.extract_symbols(file_path, content, language)?
        } else {
            Vec::new()
        };

        // Add symbols as searchable text
        let symbol_text = symbols
            .iter()
            .map(|s| format!("{} {}", s.kind.to_str(), s.name))
            .collect::<Vec<_>>()
            .join("\n");

        // Add line numbers for quick lookup
        let line_count = content.lines().count();
        let line_numbers = format!("1-{}", line_count);

        // Create document using the doc! macro
        let doc = doc!(
            self.path_field => file_path.to_string_lossy().as_ref(),
            self.content_field => content,
            self.language_field => language.to_str(),
            self.repository_field => repository,
            self.symbols_field => symbol_text.as_str(),
            self.line_numbers_field => line_numbers.as_str()
        );

        // Delete old version if exists and add new document
        if let Some(ref writer_arc) = self.writer {
            let writer = writer_arc.write().await;
            writer.delete_term(tantivy::Term::from_field_text(
                self.path_field,
                file_path.to_string_lossy().as_ref(),
            ));

            // Add new document
            writer.add_document(doc)?;
        } else {
            return Err(anyhow!("Cannot index file: indexer is read-only"));
        }

        debug!(
            "Indexed file: {:?} with {} symbols",
            file_path,
            symbols.len()
        );

        Ok(())
    }

    pub async fn delete_file(&self, file_path: &Path) -> Result<()> {
        if let Some(ref writer_arc) = self.writer {
            let writer = writer_arc.write().await;
            writer.delete_term(tantivy::Term::from_field_text(
                self.path_field,
                file_path.to_string_lossy().as_ref(),
            ));
        } else {
            return Err(anyhow!("Cannot delete file: indexer is read-only"));
        }

        debug!("Deleted file from index: {:?}", file_path);
        Ok(())
    }

    pub async fn commit(&self) -> Result<()> {
        if let Some(ref writer_arc) = self.writer {
            let mut writer = writer_arc.write().await;
            writer.commit()?;
        }

        // Reload the reader to see the latest changes
        self.reader.reload()?;

        info!("Committed index changes");
        Ok(())
    }

    pub async fn optimize(&self) -> Result<()> {
        // For now, just commit to ensure index is optimized
        // wait_merging_threads may not be available in this context
        self.commit().await?;

        info!("Optimized index");
        Ok(())
    }

    pub fn get_searcher(&self) -> tantivy::Searcher {
        self.reader.searcher()
    }

    pub fn get_schema(&self) -> &Schema {
        &self.schema
    }

    pub fn get_content_field(&self) -> Field {
        self.content_field
    }

    pub fn get_path_field(&self) -> Field {
        self.path_field
    }

    pub fn get_language_field(&self) -> Field {
        self.language_field
    }

    pub fn get_symbols_field(&self) -> Field {
        self.symbols_field
    }

    pub fn get_repository_field(&self) -> Field {
        self.repository_field
    }

    pub async fn search_documents(
        &self,
        query: &dyn tantivy::query::Query,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let searcher = self.get_searcher();
        let top_docs = searcher.search(query, &tantivy::collector::TopDocs::with_limit(limit))?;

        let mut results = Vec::new();

        for (_score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;

            let path = doc
                .get_first(self.path_field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("Missing path field"))?;

            let content = doc
                .get_first(self.content_field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("Missing content field"))?;

            let language = doc
                .get_first(self.language_field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "unknown".to_string());

            let repository = doc
                .get_first(self.repository_field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();

            results.push(SearchResult {
                path: PathBuf::from(path),
                content,
                language,
                repository,
                score: _score,
            });
        }

        Ok(results)
    }

    pub async fn get_document_count(&self) -> Result<usize> {
        let searcher = self.get_searcher();
        let count = searcher.num_docs() as usize;
        Ok(count)
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: PathBuf,
    pub content: String,
    pub language: String,
    pub repository: String,
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_tantivy_indexer() {
        let temp_dir = tempdir().unwrap();
        let index_path = temp_dir.path().join("index");

        let indexer = TantivyIndexer::new(&index_path).await.unwrap();

        // Index a test file
        let content = "fn main() { println!(\"Hello, world!\"); }";
        indexer
            .index_file(Path::new("test.rs"), "test_repo", content)
            .await
            .unwrap();

        // Commit changes
        indexer.commit().await.unwrap();

        // Verify document count
        let count = indexer.get_document_count().await.unwrap();
        assert_eq!(count, 1);

        // Search for the document
        let query_parser =
            tantivy::query::QueryParser::for_index(&indexer.index, vec![indexer.content_field]);
        let query = query_parser.parse_query("main").unwrap();

        let results = indexer.search_documents(query.as_ref(), 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, Path::new("test.rs"));
    }
}
