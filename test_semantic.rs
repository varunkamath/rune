use rune_core::search::{SearchMode, SearchQuery};
use rune_core::{Config, RuneEngine};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    println!("\n=== Testing Semantic Search ===\n");

    // Create config with semantic enabled
    let config = Config {
        workspace_dir: "/Users/varun/Projects/rune/test_workspace".to_string(),
        workspace_roots: vec![PathBuf::from("/Users/varun/Projects/rune/test_workspace")],
        cache_dir: PathBuf::from("/tmp/rune_semantic_test"),
        max_file_size: 10 * 1024 * 1024,
        indexing_threads: 4,
        enable_semantic: true,  // Enable semantic search
        languages: vec![
            "python".to_string(),
            "javascript".to_string(),
            "rust".to_string(),
            "go".to_string(),
        ],
    };

    // Create engine
    println!("Creating engine with semantic search enabled...");
    let engine = RuneEngine::new(config).await?;

    // Reindex to generate embeddings
    println!("Reindexing workspace (this will generate embeddings)...");
    engine.indexer().reindex().await?;

    // Get stats
    let stats = engine.stats().await?;
    println!("\nIndex stats: {:?}\n", stats);

    // Test 1: Semantic search for mathematical concepts
    println!("Test 1: Semantic search for 'mathematical calculations'");
    let query = SearchQuery {
        query: "mathematical calculations".to_string(),
        mode: SearchMode::Semantic,
        repositories: None,
        file_patterns: None,
        limit: 5,
        offset: 0,
    };

    let results = engine.search().search(query).await?;
    println!("Found {} semantic results", results.results.len());
    for (i, result) in results.results.iter().enumerate() {
        println!("  {}. {}:{} - {}",
            i + 1,
            result.file_path.display(),
            result.line_number,
            &result.content[..result.content.len().min(80)]
        );
    }

    // Test 2: Semantic search for data structures
    println!("\nTest 2: Semantic search for 'data structures arrays lists'");
    let query = SearchQuery {
        query: "data structures arrays lists".to_string(),
        mode: SearchMode::Semantic,
        repositories: None,
        file_patterns: None,
        limit: 5,
        offset: 0,
    };

    let results = engine.search().search(query).await?;
    println!("Found {} semantic results", results.results.len());
    for (i, result) in results.results.iter().enumerate() {
        println!("  {}. {}:{} - {}",
            i + 1,
            result.file_path.display(),
            result.line_number,
            &result.content[..result.content.len().min(80)]
        );
    }

    // Test 3: Hybrid search combining literal and semantic
    println!("\nTest 3: Hybrid search for 'function'");
    let query = SearchQuery {
        query: "function".to_string(),
        mode: SearchMode::Hybrid,
        repositories: None,
        file_patterns: None,
        limit: 5,
        offset: 0,
    };

    let results = engine.search().search(query).await?;
    println!("Found {} hybrid results", results.results.len());
    for (i, result) in results.results.iter().enumerate() {
        println!("  {}. {}:{} - {}",
            i + 1,
            result.file_path.display(),
            result.line_number,
            &result.content[..result.content.len().min(80)]
        );
    }

    println!("\n=== Semantic Search Test Complete ===\n");
    Ok(())
}
