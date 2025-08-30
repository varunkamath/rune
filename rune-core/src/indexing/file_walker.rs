use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use ignore::WalkBuilder;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

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

    pub fn watch_directory(&self, root: &Path, tx: mpsc::Sender<FileEvent>) -> Result<()> {
        use notify::{EventKind, RecursiveMode, Watcher};
        use std::sync::mpsc as std_mpsc;

        let root_path = root.to_path_buf();
        let (event_tx, event_rx) = std_mpsc::channel();

        // Create a simple watcher instead of debouncer for now
        let mut watcher =
            notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = event_tx.send(event);
                }
            })?;

        watcher.watch(&root_path, RecursiveMode::Recursive)?;

        // Process events in a separate thread
        std::thread::spawn(move || {
            while let Ok(event) = event_rx.recv() {
                for path in event.paths {
                    if !Self::is_indexable_file(&path) {
                        continue;
                    }

                    let file_event = match event.kind {
                        EventKind::Create(_) => FileEvent::Created(path),
                        EventKind::Modify(_) => FileEvent::Modified(path),
                        EventKind::Remove(_) => FileEvent::Deleted(path),
                        _ => continue,
                    };

                    if tx.blocking_send(file_event).is_err() {
                        break;
                    }
                }
            }
        });

        // Keep the watcher alive
        std::mem::forget(watcher);

        Ok(())
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
