use rune_core::search::{SearchMode, SearchQuery};
use rune_core::{Config, RuneEngine};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("debug").init();

    println!("Testing search directly...");

    // Create config
    let config = Config {
        workspace_dir: "/Users/varun/Projects/rune/test_workspace".to_string(),
        workspace_roots: vec![PathBuf::from("/Users/varun/Projects/rune/test_workspace")],
        cache_dir: PathBuf::from("/Users/varun/Projects/rune/mcp-server/.rune_cache"),
        max_file_size: 10 * 1024 * 1024,
        indexing_threads: 4,
        enable_semantic: true,
        languages: vec![
            "python".to_string(),
            "javascript".to_string(),
            "rust".to_string(),
            "go".to_string(),
        ],
        file_watch_debounce_ms: 500,
    };

    // Create engine
    let engine = RuneEngine::new(config).await?;

    // Reindex
    println!("Reindexing workspace...");
    engine.indexer().reindex().await?;

    // Get stats
    let stats = engine.stats().await?;
    println!("Stats: {:?}", stats);

    // Test symbol search
    let query = SearchQuery {
        query: "main".to_string(),
        mode: SearchMode::Symbol,
        repositories: None,
        file_patterns: None,
        limit: 10,
        offset: 0,
    };

    println!("Searching for 'main' with symbol mode...");
    let results = engine.search().search(query).await?;

    println!("Found {} results", results.results.len());
    for result in &results.results {
        println!(
            "  - {}:{} - {}",
            result.file_path.display(),
            result.line_number,
            result.content
        );
    }

    Ok(())
}
