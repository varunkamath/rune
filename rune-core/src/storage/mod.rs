use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use bincode::{Decode, Encode};
use parking_lot::RwLock;
use rocksdb::{DB, Options};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct StorageBackend {
    db: Arc<RwLock<DB>>,
    cache_dir: PathBuf,
}

impl StorageBackend {
    pub async fn new(cache_dir: &Path) -> Result<Self> {
        // Create cache directory if it doesn't exist
        tokio::fs::create_dir_all(cache_dir).await?;

        let db_path = cache_dir.join("metadata.db");

        // Try to recover from stale lock if necessary
        Self::try_recover_lock(&db_path)?;

        // Open RocksDB
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_max_open_files(1000);
        opts.set_use_fsync(false);
        opts.set_bytes_per_sync(1048576);

        let db = DB::open(&opts, db_path)?;

        Ok(Self {
            db: Arc::new(RwLock::new(db)),
            cache_dir: cache_dir.to_path_buf(),
        })
    }

    pub async fn list_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let db = self.db.read();
        let iter = db.iterator(rocksdb::IteratorMode::Start);

        for item in iter {
            if let Ok((key, _)) = item
                && let Ok(path_str) = std::str::from_utf8(&key)
            {
                files.push(PathBuf::from(path_str));
            }
        }

        Ok(files)
    }

    pub async fn get_file_count(&self) -> Result<usize> {
        let files = self.list_files().await?;
        Ok(files.len())
    }

    pub async fn get_symbol_count(&self) -> Result<usize> {
        // Count symbols from all indexed files
        // This is an approximation based on average symbols per file
        let file_count = self.get_file_count().await?;
        // Estimate average of 20 symbols per file (can be refined with actual parsing)
        Ok(file_count * 20)
    }

    pub async fn get_index_size(&self) -> Result<u64> {
        // Calculate the size of the Tantivy index directory
        let index_path = self.cache_dir.join("tantivy_index");
        let size = self.calculate_directory_size(&index_path).await?;
        Ok(size)
    }

    pub async fn get_cache_size(&self) -> Result<u64> {
        // Calculate the total size of the cache directory
        let size = self.calculate_directory_size(&self.cache_dir).await?;
        Ok(size)
    }

    pub async fn store_file_metadata(
        &self,
        file_path: &Path,
        metadata: FileMetadata,
    ) -> Result<()> {
        let key = file_path.to_string_lossy().as_bytes().to_vec();
        let config = bincode::config::standard();
        let value = bincode::encode_to_vec(&metadata, config)?;

        let db = self.db.write();
        db.put(key, value)?;

        Ok(())
    }

    pub async fn delete_file_metadata(&self, file_path: &Path) -> Result<()> {
        let key = file_path.to_string_lossy().as_bytes().to_vec();

        let db = self.db.write();
        db.delete(key)?;

        Ok(())
    }

    pub async fn get_file_metadata(&self, file_path: &Path) -> Result<Option<FileMetadata>> {
        let key = file_path.to_string_lossy().as_bytes().to_vec();

        let db = self.db.read();
        match db.get(key)? {
            Some(value) => {
                let config = bincode::config::standard();
                let (metadata, _) = bincode::decode_from_slice(&value, config)?;
                Ok(Some(metadata))
            },
            None => Ok(None),
        }
    }

    async fn calculate_directory_size(&self, path: &Path) -> Result<u64> {
        let mut total_size = 0u64;

        if !path.exists() {
            return Ok(0);
        }

        let mut entries = tokio::fs::read_dir(path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;

            if metadata.is_file() {
                total_size += metadata.len();
            } else if metadata.is_dir() {
                // Recursively calculate subdirectory size
                let subdir_size = Box::pin(self.calculate_directory_size(&entry.path())).await?;
                total_size += subdir_size;
            }
        }

        Ok(total_size)
    }

    /// Try to recover from a stale lock file
    pub fn try_recover_lock(db_path: &Path) -> Result<()> {
        use tracing::{debug, warn};

        let lock_path = db_path.join("LOCK");

        if lock_path.exists() {
            // Check if we can open the database - if another process has it, this will fail
            match DB::open_for_read_only(&Options::default(), db_path, false) {
                Ok(_) => {
                    // Database is not actually locked, remove stale LOCK file
                    warn!("Removing stale RocksDB LOCK file at {:?}", lock_path);
                    std::fs::remove_file(&lock_path)?;
                    Ok(())
                },
                Err(_) => {
                    // Database is genuinely locked by another process
                    debug!("RocksDB is locked by another process");
                    Ok(())
                },
            }
        } else {
            Ok(())
        }
    }
}

impl Drop for StorageBackend {
    fn drop(&mut self) {
        // RocksDB will be properly closed when the Arc<RwLock<DB>> is dropped
        // This ensures the LOCK file is released even on abnormal termination
        use tracing::debug;
        debug!("Closing RocksDB connection for {:?}", self.cache_dir);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub size: u64,
    pub modified: u64,
    pub language: String,
    pub hash: String,
    pub indexed_at: u64,
}
