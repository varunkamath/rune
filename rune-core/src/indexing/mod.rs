use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::{Config, storage::StorageBackend};

pub struct Indexer {
    config: Arc<Config>,
    storage: StorageBackend,
    watcher_handle: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl Indexer {
    pub async fn new(config: Arc<Config>, storage: StorageBackend) -> Result<Self> {
        Ok(Self {
            config,
            storage,
            watcher_handle: None,
            shutdown_tx: None,
        })
    }

    pub async fn start_watching(&mut self) -> Result<()> {
        if self.watcher_handle.is_some() {
            warn!("File watcher already running");
            return Ok(());
        }

        info!("Starting file watcher");

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        let _config = self.config.clone();
        let _storage = self.storage.clone();

        let handle = tokio::spawn(async move {
            // TODO: Implement file watching with notify crate
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("File watcher shutting down");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                        // Check for file changes
                    }
                }
            }
        });

        self.watcher_handle = Some(handle);
        self.shutdown_tx = Some(shutdown_tx);

        Ok(())
    }

    pub async fn stop_watching(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        if let Some(handle) = self.watcher_handle.take() {
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

        Ok(())
    }

    async fn index_directory(&self, path: &PathBuf) -> Result<()> {
        // TODO: Implement directory indexing
        info!("Indexing directory: {:?}", path);
        Ok(())
    }

    pub async fn reindex(&self) -> Result<()> {
        info!("Reindexing all workspaces");
        self.index_workspaces().await
    }
}
