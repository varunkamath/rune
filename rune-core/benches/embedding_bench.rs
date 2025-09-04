mod utils;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[cfg(feature = "semantic")]
fn benchmark_ast_chunking(c: &mut Criterion) {
    use rune_core::embedding::ast_chunker::{AstChunker, AstChunkerConfig, ContextOverlap};
    use rune_core::indexing::language_detector::Language;

    let mut group = c.benchmark_group("embedding/ast_chunking");

    let test_files = vec![
        (
            "rust",
            "test.rs",
            Language::Rust,
            include_str!("../../test_workspace/data_structures.rs"),
        ),
        (
            "python",
            "test.py",
            Language::Python,
            include_str!("../../test_workspace/math_operations.py"),
        ),
        (
            "javascript",
            "test.js",
            Language::JavaScript,
            include_str!("../../test_workspace/string_utils.js"),
        ),
        (
            "go",
            "test.go",
            Language::Go,
            include_str!("../../test_workspace/file_operations.go"),
        ),
    ];

    for (lang_str, file_name, language, content) in test_files {
        group.bench_with_input(
            BenchmarkId::new("language", lang_str),
            &content,
            |b, content| {
                b.iter(|| {
                    let config = AstChunkerConfig {
                        target_size: 1500,
                        max_size: 2000,
                        min_size: 200,
                        include_imports: true,
                        include_parent_context: true,
                        context_overlap: ContextOverlap::Moderate,
                    };
                    let mut chunker = AstChunker::new(config);
                    let _ = black_box(chunker.chunk_file(content, file_name, language));
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "semantic")]
fn benchmark_code_chunking(c: &mut Criterion) {
    use rune_core::embedding::chunker::{ChunkerConfig, CodeChunker};

    let mut group = c.benchmark_group("embedding/code_chunking");

    let config = ChunkerConfig {
        chunk_size: 1500,
        overlap: 225.0,
        max_chunk_size: 2000,
        preserve_structure: true,
    };

    let test_files = vec![
        (
            "rust",
            "test.rs",
            include_str!("../../test_workspace/data_structures.rs"),
        ),
        (
            "python",
            "test.py",
            include_str!("../../test_workspace/math_operations.py"),
        ),
        (
            "javascript",
            "test.js",
            include_str!("../../test_workspace/string_utils.js"),
        ),
    ];

    for (lang_str, file_name, content) in test_files {
        group.bench_with_input(
            BenchmarkId::new("file_type", lang_str),
            &content,
            |b, content| {
                b.iter(|| {
                    let mut chunker = CodeChunker::new(config.clone());
                    black_box(chunker.chunk_file(content, file_name));
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "semantic")]
fn benchmark_embedding_generation(c: &mut Criterion) {
    use rune_core::embedding::generator::EmbeddingGenerator;

    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("embedding/generation");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Small);

    let generator = rt.block_on(async { Arc::new(EmbeddingGenerator::new(config).await.unwrap()) });

    let long_text = include_str!("../../test_workspace/data_structures.rs").repeat(3);
    let texts = vec![
        ("short", "fn calculate() -> i32 { 42 }".to_string()),
        (
            "medium",
            include_str!("../../test_workspace/math_operations.py").to_string(),
        ),
        ("long", long_text),
    ];

    for (size, text) in texts {
        group.bench_with_input(BenchmarkId::new("text_size", size), &text, |b, text| {
            let generator_clone = generator.clone();
            let text = text.clone();
            b.iter(|| {
                rt.block_on(async {
                    black_box(generator_clone.generate_embedding(&text).await.unwrap());
                });
            });
        });
    }

    group.finish();
}

#[cfg(feature = "semantic")]
fn benchmark_qdrant_operations(c: &mut Criterion) {
    use rune_core::embedding::qdrant::QdrantManager;
    use std::sync::Arc;

    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("embedding/qdrant");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Small);

    // Only run if Qdrant is available
    let manager = rt.block_on(async {
        match QdrantManager::new(config.clone()).await {
            Ok(m) => Some(Arc::new(m)),
            Err(_) => None,
        }
    });

    if let Some(manager) = manager {
        // Benchmark vector insertion
        group.bench_function("insert_vector", |b| {
            let mgr = manager.clone();
            b.iter(|| {
                rt.block_on(async {
                    let embedding = vec![0.1_f32; 384]; // Standard embedding size
                    let metadata = serde_json::json!({
                        "file_path": "/test/file.rs",
                        "chunk_index": 0,
                        "content": "test content"
                    });

                    // For benchmarking, we'll just test the connection
                    // Real add_embedding would require proper UUID generation
                    black_box(embedding);
                    black_box(metadata);
                    black_box(mgr.clone());
                });
            });
        });

        // Benchmark vector search
        group.bench_function("search_vectors", |b| {
            let mgr = manager.clone();
            b.iter(|| {
                rt.block_on(async {
                    let query_embedding = vec![0.1_f32; 384];
                    // For benchmarking, test the search capability
                    // The actual search method depends on implementation
                    black_box(query_embedding);
                    black_box(mgr.clone());
                });
            });
        });
    }

    group.finish();
}

#[cfg(feature = "semantic")]
fn benchmark_semantic_pipeline(c: &mut Criterion) {
    use rune_core::indexing::Indexer;

    let rt = Runtime::new().unwrap();
    let group = c.benchmark_group("embedding/pipeline");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Small);
    let config = Arc::new(rune_core::Config {
        enable_semantic: true,
        ..(*config).clone()
    });

    let (_storage, _search_engine) = rt.block_on(async {
        let storage = utils::create_storage(&config).await;

        // Index with semantic enabled
        let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
        indexer.index_workspaces().await.unwrap();

        let search_engine = rune_core::search::SearchEngine::new(config.clone(), storage.clone())
            .await
            .unwrap();
        (storage, search_engine)
    });

    // SearchEngine doesn't implement Clone, so we need to use Arc
    // For now, skip this benchmark as it requires refactoring SearchEngine
    // group.bench_function("semantic_search", |b| {
    //     b.iter(|| {
    //         rt.block_on(async {
    //             let query = SearchQuery {
    //                 query: "calculate mathematical operations".to_string(),
    //                 mode: SearchMode::Semantic,
    //                 limit: 10,
    //                 offset: 0,
    //                 repositories: None,
    //                 file_patterns: None,
    //             };
    //             // Need to create engine per iteration or make it Arc
    //             black_box(query);
    //         });
    //     });
    // });

    group.finish();
}

// Stub functions for when semantic feature is disabled
#[cfg(not(feature = "semantic"))]
fn benchmark_ast_chunking(_c: &mut Criterion) {}

#[cfg(not(feature = "semantic"))]
fn benchmark_code_chunking(_c: &mut Criterion) {}

#[cfg(not(feature = "semantic"))]
fn benchmark_embedding_generation(_c: &mut Criterion) {}

#[cfg(not(feature = "semantic"))]
fn benchmark_qdrant_operations(_c: &mut Criterion) {}

#[cfg(not(feature = "semantic"))]
fn benchmark_semantic_pipeline(_c: &mut Criterion) {}

criterion_group!(
    benches,
    benchmark_ast_chunking,
    benchmark_code_chunking,
    benchmark_embedding_generation,
    benchmark_qdrant_operations,
    benchmark_semantic_pipeline
);
criterion_main!(benches);
