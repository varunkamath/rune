pub mod file_walker;
pub mod language_detector;
pub mod symbol_extractor;
pub mod tantivy_indexer;

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use rayon::prelude::*;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use self::file_walker::{FileEvent, FileWalker};
use self::tantivy_indexer::TantivyIndexer;
use crate::{Config, storage::StorageBackend};

pub struct Indexer {
    config: Arc<Config>,
    storage: StorageBackend,
    tantivy_indexer: Arc<TantivyIndexer>,
    file_walker: FileWalker,
    watcher_handles: Vec<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl Indexer {
    pub async fn new(config: Arc<Config>, storage: StorageBackend) -> Result<Self> {
        let index_path = config.cache_dir.join("tantivy_index");
        let tantivy_indexer = Arc::new(TantivyIndexer::new(&index_path).await?);
        let file_walker = FileWalker::new(config.clone());

        Ok(Self {
            config,
            storage,
            tantivy_indexer,
            file_walker,
            watcher_handles: Vec::new(),
            shutdown_tx: None,
        })
    }

    pub async fn start_watching(&mut self) -> Result<()> {
        if !self.watcher_handles.is_empty() {
            warn!("File watchers already running");
            return Ok(());
        }

        info!(
            "Starting file watchers for {} workspaces",
            self.config.workspace_roots.len()
        );

        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        let (event_tx, mut event_rx) = mpsc::channel(1000);

        // Start a watcher for each workspace root
        for root in &self.config.workspace_roots {
            let root = root.clone();
            let event_tx = event_tx.clone();

            let handle = tokio::spawn(async move {
                if let Err(e) =
                    FileWalker::new(Arc::new(Config::default())).watch_directory(&root, event_tx)
                {
                    error!("Failed to watch directory {:?}: {}", root, e);
                }
            });

            self.watcher_handles.push(handle);
        }

        // Start event processor
        let tantivy_indexer = self.tantivy_indexer.clone();
        let storage = self.storage.clone();
        let mut shutdown_rx = shutdown_rx;

        let processor_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(event) = event_rx.recv() => {
                        if let Err(e) = Self::process_file_event(
                            event,
                            &tantivy_indexer,
                            &storage,
                        ).await {
                            error!("Failed to process file event: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("File event processor shutting down");
                        break;
                    }
                }
            }
        });

        self.watcher_handles.push(processor_handle);
        self.shutdown_tx = Some(shutdown_tx);

        Ok(())
    }

    pub async fn stop_watching(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        for handle in self.watcher_handles.drain(..) {
            handle.await?;
        }

        Ok(())
    }

    pub async fn index_workspaces(&self) -> Result<()> {
        info!(
            "Indexing {} workspace roots",
            self.config.workspace_roots.len()
        );

        for root in &self.config.workspace_roots {
            self.index_directory(root).await?;
        }

        // Commit all changes
        self.tantivy_indexer.commit().await?;

        info!("Indexing complete");
        Ok(())
    }

    async fn index_directory(&self, path: &Path) -> Result<()> {
        info!("Indexing directory: {:?}", path);

        let files = self.file_walker.walk_directory(path).await?;
        let total_files = files.len();

        info!("Found {} files to index", total_files);

        // Get repository name from path
        let repository = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Process files in parallel batches
        let batch_size = 100;
        let tantivy_indexer = self.tantivy_indexer.clone();
        let storage = self.storage.clone();

        for (batch_num, batch) in files.chunks(batch_size).enumerate() {
            let batch_files: Vec<_> = batch.to_vec();

            // Process batch in parallel using rayon
            let results: Vec<_> = batch_files
                .par_iter()
                .map(|file_path| {
                    // Read file content
                    match std::fs::read_to_string(file_path) {
                        Ok(content) => {
                            // Create a future for indexing
                            (file_path.clone(), repository.to_string(), content)
                        },
                        Err(e) => {
                            warn!("Failed to read file {:?}: {}", file_path, e);
                            (file_path.clone(), repository.to_string(), String::new())
                        },
                    }
                })
                .collect();

            // Index all files in the batch
            for (file_path, repo, content) in results {
                if !content.is_empty() {
                    if let Err(e) = tantivy_indexer
                        .index_file(&file_path, &repo, &content)
                        .await
                    {
                        error!("Failed to index file {:?}: {}", file_path, e);
                    }

                    // Store metadata
                    let metadata = crate::storage::FileMetadata {
                        path: file_path.clone(),
                        size: content.len() as u64,
                        modified: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        language: language_detector::LanguageDetector::detect(
                            &file_path,
                            Some(&content),
                        )
                        .to_str()
                        .to_string(),
                        hash: blake3::hash(content.as_bytes()).to_string(),
                        indexed_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };

                    if let Err(e) = storage.store_file_metadata(&file_path, metadata).await {
                        error!("Failed to store metadata for {:?}: {}", file_path, e);
                    }
                }
            }

            // Commit periodically
            if batch_num % 10 == 0 {
                tantivy_indexer.commit().await?;
                debug!(
                    "Indexed {} / {} files",
                    (batch_num + 1) * batch_size,
                    total_files
                );
            }
        }

        Ok(())
    }

    async fn process_file_event(
        event: FileEvent,
        tantivy_indexer: &TantivyIndexer,
        storage: &StorageBackend,
    ) -> Result<()> {
        match event {
            FileEvent::Created(path) | FileEvent::Modified(path) => {
                // Read file content
                let content = tokio::fs::read_to_string(&path).await?;

                // Get repository name
                let repository = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                // Index file
                tantivy_indexer
                    .index_file(&path, repository, &content)
                    .await?;

                // Store metadata
                let metadata = crate::storage::FileMetadata {
                    path: path.clone(),
                    size: content.len() as u64,
                    modified: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    language: language_detector::LanguageDetector::detect(&path, Some(&content))
                        .to_str()
                        .to_string(),
                    hash: blake3::hash(content.as_bytes()).to_string(),
                    indexed_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                storage.store_file_metadata(&path, metadata).await?;

                // Commit changes
                tantivy_indexer.commit().await?;

                info!("Indexed file: {:?}", path);
            },
            FileEvent::Deleted(path) => {
                // Remove from index
                tantivy_indexer.delete_file(&path).await?;
                tantivy_indexer.commit().await?;

                info!("Removed file from index: {:?}", path);
            },
        }

        Ok(())
    }

    pub async fn reindex(&self) -> Result<()> {
        info!("Reindexing all workspaces");

        // Clear existing index
        // Note: In production, you might want to build a new index and swap

        // Reindex everything
        self.index_workspaces().await?;

        // Optimize index after bulk reindexing
        self.tantivy_indexer.optimize().await?;

        Ok(())
    }

    pub fn get_tantivy_indexer(&self) -> &TantivyIndexer {
        &self.tantivy_indexer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_indexer() {
        let temp_dir = tempdir().unwrap();
        let workspace = temp_dir.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();

        // Create test files
        std::fs::write(workspace.join("test.rs"), "fn main() {}").unwrap();
        std::fs::write(workspace.join("test.py"), "def main(): pass").unwrap();

        let config = Arc::new(Config {
            workspace_roots: vec![workspace],
            cache_dir: temp_dir.path().join("cache"),
            ..Default::default()
        });

        let storage = StorageBackend::new(&config.cache_dir).await.unwrap();
        let indexer = Indexer::new(config, storage).await.unwrap();

        // Index workspaces
        indexer.index_workspaces().await.unwrap();

        // Verify files were indexed
        let doc_count = indexer.tantivy_indexer.get_document_count().await.unwrap();
        assert_eq!(doc_count, 2);
    }
}
