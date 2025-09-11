use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use ignore::WalkBuilder;
use notify::{Config as NotifyConfig, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, FileIdMap, new_debouncer_opt};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::Config;

pub struct FileWalker {
    config: Arc<Config>,
}

impl FileWalker {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    pub async fn walk_workspaces(&self) -> Result<Vec<PathBuf>> {
        let mut all_files = Vec::new();

        for root in &self.config.workspace_roots {
            info!("Walking workspace: {:?}", root);
            let files = self.walk_directory(root).await?;
            all_files.extend(files);
        }

        info!("Found {} files across all workspaces", all_files.len());
        Ok(all_files)
    }

    pub async fn walk_directory(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let (tx, mut rx) = mpsc::channel(1000);
        let root = root.to_path_buf();
        let max_file_size = self.config.max_file_size;

        // Spawn blocking task for file walking
        let handle = tokio::task::spawn_blocking(move || {
            let walker = WalkBuilder::new(&root)
                .hidden(false) // Include hidden files
                .git_ignore(true) // Respect .gitignore
                .git_global(true) // Respect global gitignore
                .git_exclude(true) // Respect .git/info/exclude
                .require_git(false) // Don't require git repo
                .ignore(true) // Respect .ignore files
                .max_filesize(Some(max_file_size as u64))
                .build();

            for entry in walker {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();

                        // Skip directories
                        if path.is_dir() {
                            continue;
                        }

                        // Skip binary files and non-text files
                        if !Self::is_indexable_file(path) {
                            continue;
                        }

                        if tx.blocking_send(path.to_path_buf()).is_err() {
                            break; // Receiver dropped
                        }
                    },
                    Err(e) => {
                        warn!("Error walking directory: {}", e);
                    },
                }
            }
        });

        // Collect results
        let mut files = Vec::new();
        while let Some(path) = rx.recv().await {
            files.push(path);
        }

        handle.await?;

        debug!("Found {} files", files.len());
        Ok(files)
    }

    pub fn watch_directory(
        &self,
        root: &Path,
        tx: mpsc::Sender<FileEvent>,
        debounce_ms: u64,
    ) -> Result<Debouncer<notify::RecommendedWatcher, FileIdMap>> {
        use std::sync::mpsc as std_mpsc;

        let root_path = root.to_path_buf();
        let (event_tx, event_rx) = std_mpsc::channel();

        // Create a debounced watcher with FileIdMap cache
        let mut debouncer = new_debouncer_opt(
            Duration::from_millis(debounce_ms),
            None, // No tick rate
            move |result: DebounceEventResult| {
                if let Ok(events) = result {
                    for event in events {
                        let _ = event_tx.send(event);
                    }
                }
            },
            FileIdMap::new(),
            NotifyConfig::default(),
        )?;

        // Start watching the directory
        debouncer.watch(&root_path, RecursiveMode::Recursive)?;

        info!(
            "Started watching directory {:?} with {}ms debounce",
            root_path, debounce_ms
        );

        // Process debounced events in a separate thread
        std::thread::spawn(move || {
            while let Ok(event) = event_rx.recv() {
                let paths = event.paths.clone();
                let kind = event.kind;

                for path in paths {
                    if !Self::is_indexable_file(&path) {
                        continue;
                    }

                    use notify::EventKind;
                    let file_event = match kind {
                        EventKind::Create(_) => FileEvent::Created(path),
                        EventKind::Modify(_) => FileEvent::Modified(path),
                        EventKind::Remove(_) => FileEvent::Deleted(path),
                        _ => continue,
                    };

                    debug!("Debounced file event: {:?}", file_event);
                    if tx.blocking_send(file_event).is_err() {
                        error!("Failed to send file event, receiver dropped");
                        break;
                    }
                }
            }
            info!("File watcher thread terminating");
        });

        Ok(debouncer)
    }

    fn is_indexable_file(path: &Path) -> bool {
        // Check if file has a text extension
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();

            // Common source code extensions
            matches!(
                ext.as_str(),
                "rs" | "js"
                    | "jsx"
                    | "ts"
                    | "tsx"
                    | "py"
                    | "go"
                    | "java"
                    | "cpp"
                    | "c"
                    | "h"
                    | "hpp"
                    | "cs"
                    | "rb"
                    | "php"
                    | "swift"
                    | "kt"
                    | "scala"
                    | "r"
                    | "jl"
                    | "lua"
                    | "dart"
                    | "elm"
                    | "clj"
                    | "ex"
                    | "exs"
                    | "erl"
                    | "hrl"
                    | "hs"
                    | "ml"
                    | "vim"
                    | "sh"
                    | "bash"
                    | "zsh"
                    | "fish"
                    | "ps1"
                    | "bat"
                    | "sql"
                    | "md"
                    | "txt"
                    | "yml"
                    | "yaml"
                    | "toml"
                    | "json"
                    | "xml"
                    | "html"
                    | "css"
                    | "scss"
                    | "sass"
                    | "less"
                    | "vue"
                    | "svelte"
                    | "astro"
            )
        } else {
            // Check for files without extensions (like shell scripts)
            if let Some(name) = path.file_name() {
                let name = name.to_string_lossy();
                matches!(
                    name.as_ref(),
                    "Makefile"
                        | "Dockerfile"
                        | "Gemfile"
                        | "Rakefile"
                        | "Procfile"
                        | "Vagrantfile"
                        | ".gitignore"
                        | ".dockerignore"
                        | "LICENSE"
                        | "README"
                )
            } else {
                false
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum FileEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio;

    #[tokio::test]
    async fn test_file_walker() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        std::fs::write(&test_file, "fn main() {}").unwrap();

        let config = Arc::new(Config {
            workspace_roots: vec![temp_dir.path().to_path_buf()],
            ..Default::default()
        });

        let walker = FileWalker::new(config);
        let files = walker.walk_workspaces().await.unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0], test_file);
    }

    #[test]
    fn test_is_indexable_file() {
        assert!(FileWalker::is_indexable_file(Path::new("test.rs")));
        assert!(FileWalker::is_indexable_file(Path::new("main.py")));
        assert!(FileWalker::is_indexable_file(Path::new("Dockerfile")));
        assert!(!FileWalker::is_indexable_file(Path::new("image.png")));
        assert!(!FileWalker::is_indexable_file(Path::new("binary.exe")));
    }
}
