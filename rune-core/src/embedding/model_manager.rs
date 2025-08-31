use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::info;

const MODEL_NAME: &str = "all-MiniLM-L6-v2";
const MODEL_FILES: &[(&str, &str)] = &[
    (
        "model.onnx",
        "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx",
    ),
    (
        "tokenizer.json",
        "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json",
    ),
    (
        "tokenizer_config.json",
        "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer_config.json",
    ),
];

/// Manages embedding model downloads and caching
pub struct ModelManager {
    cache_dir: PathBuf,
}

impl ModelManager {
    /// Create a new model manager with default cache directory
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("rune")
            .join("models")
            .join(MODEL_NAME);

        Ok(Self { cache_dir })
    }

    /// Create a model manager with a custom cache directory (for testing)
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir: cache_dir.join("models").join(MODEL_NAME),
        }
    }

    /// Get the path to the model directory, downloading if necessary
    pub async fn get_model_path(&self) -> Result<PathBuf> {
        // Check if model already exists
        if self.is_model_cached() {
            info!("Using cached model at {:?}", self.cache_dir);
            return Ok(self.cache_dir.clone());
        }

        info!("Model not found in cache, downloading...");
        self.download_model().await?;
        Ok(self.cache_dir.clone())
    }

    /// Check if the model is already cached
    pub fn is_model_cached(&self) -> bool {
        // Check if all required files exist
        MODEL_FILES
            .iter()
            .all(|(filename, _)| self.cache_dir.join(filename).exists())
    }

    /// Download the model from HuggingFace
    async fn download_model(&self) -> Result<()> {
        // Create cache directory
        fs::create_dir_all(&self.cache_dir).context("Failed to create model cache directory")?;

        // Download each file
        for (filename, url) in MODEL_FILES {
            let file_path = self.cache_dir.join(filename);

            if file_path.exists() {
                info!("File {} already exists, skipping", filename);
                continue;
            }

            info!("Downloading {} from {}", filename, url);
            self.download_file(url, &file_path)
                .await
                .with_context(|| format!("Failed to download {}", filename))?;
        }

        info!("Model download complete");
        Ok(())
    }

    /// Download a single file
    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .send()
            .await
            .context("Failed to send download request")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status: {}", response.status());
        }

        // Get total size for progress tracking
        let total_size = response.content_length().unwrap_or(0);

        // Create temporary file
        let temp_path = dest.with_extension("tmp");
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .context("Failed to create temporary file")?;

        // Download with progress tracking
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read chunk")?;
            file.write_all(&chunk)
                .await
                .context("Failed to write chunk")?;

            downloaded += chunk.len() as u64;

            if total_size > 0 {
                let progress = (downloaded as f64 / total_size as f64) * 100.0;
                if downloaded % (1024 * 1024) == 0 || downloaded == total_size {
                    info!("Download progress: {:.1}%", progress);
                }
            }
        }

        file.flush().await.context("Failed to flush file")?;
        drop(file);

        // Move temp file to final location
        tokio::fs::rename(&temp_path, dest)
            .await
            .context("Failed to move downloaded file")?;

        Ok(())
    }

    /// Get the path to a specific model file
    pub fn get_file_path(&self, filename: &str) -> PathBuf {
        self.cache_dir.join(filename)
    }

    /// Clear the model cache
    pub fn clear_cache(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).context("Failed to remove cache directory")?;
            info!("Model cache cleared");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_model_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ModelManager::with_cache_dir(temp_dir.path().to_path_buf());

        assert!(!manager.is_model_cached());
        assert!(manager.cache_dir.to_string_lossy().contains(MODEL_NAME));
    }

    #[test]
    fn test_cache_detection() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ModelManager::with_cache_dir(temp_dir.path().to_path_buf());

        // Create fake model files
        fs::create_dir_all(&manager.cache_dir).unwrap();
        for (filename, _) in MODEL_FILES {
            fs::write(manager.cache_dir.join(filename), "test").unwrap();
        }

        assert!(manager.is_model_cached());
    }
}
