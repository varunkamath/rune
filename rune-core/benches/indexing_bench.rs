mod utils;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rune_core::indexing::Indexer;
use std::hint::black_box;
use std::sync::Arc;
use tokio::runtime::Runtime;

fn benchmark_file_indexing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("indexing/files");

    // Only benchmark with Small dataset to keep runtime reasonable
    let size = utils::DatasetSize::Small;

    // Setup workspace once outside the benchmark
    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(size);

    group.bench_function("index_files_small", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Create fresh storage for each iteration
                let storage = utils::create_storage(&config).await;
                let indexer = Indexer::new(config.clone(), storage).await.unwrap();
                let _: () = indexer.index_workspaces().await.unwrap();
                black_box(());
            });
        });
    });

    group.finish();
}

fn benchmark_tantivy_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("indexing/tantivy");

    // Benchmark file indexing through TantivyIndexer
    group.bench_function("index_file", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (_temp, workspace, config) =
                    utils::setup_benchmark_workspace(utils::DatasetSize::Small);
                let index_path = config.cache_dir.join("tantivy_index");
                let indexer =
                    rune_core::indexing::tantivy_indexer::TantivyIndexer::new(&index_path)
                        .await
                        .unwrap();

                // Read a sample file
                let file_path = workspace.join("file_0.rs");
                let content = std::fs::read_to_string(&file_path).unwrap();

                let _: () = indexer
                    .index_file(&file_path, "workspace", &content)
                    .await
                    .unwrap();
                black_box(());
            });
        });
    });

    // Benchmark commit operation
    group.bench_function("commit", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (_temp, workspace, config) =
                    utils::setup_benchmark_workspace(utils::DatasetSize::Small);
                let index_path = config.cache_dir.join("tantivy_index");
                let indexer =
                    rune_core::indexing::tantivy_indexer::TantivyIndexer::new(&index_path)
                        .await
                        .unwrap();

                // Index some files
                for i in 0..5 {
                    let file_path = workspace.join(format!("file_{}.rs", i % 4));
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        let _ = indexer.index_file(&file_path, "workspace", &content).await;
                    }
                }

                let _: () = indexer.commit().await.unwrap();
                black_box(());
            });
        });
    });

    group.finish();
}

fn benchmark_symbol_extraction(c: &mut Criterion) {
    use rune_core::indexing::language_detector::Language;
    use rune_core::indexing::symbol_extractor::SymbolExtractor;
    use std::path::Path;

    let mut group = c.benchmark_group("indexing/symbols");

    let languages = vec![
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
        (
            "go",
            "test.go",
            include_str!("../../test_workspace/file_operations.go"),
        ),
    ];

    for (lang_str, file_name, content) in languages {
        group.bench_with_input(
            BenchmarkId::new("extract", lang_str),
            &content,
            |b, &content| {
                b.iter(|| {
                    let extractor = SymbolExtractor::new();
                    let path = Path::new(file_name);
                    let lang = Language::from_extension(
                        path.extension().and_then(|s| s.to_str()).unwrap_or(""),
                    );
                    let _ = black_box(extractor.extract_symbols(path, content, lang));
                });
            },
        );
    }

    group.finish();
}

fn benchmark_incremental_indexing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("indexing/incremental");

    group.bench_function("detect_changes", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (_temp, workspace, config) =
                    utils::setup_benchmark_workspace(utils::DatasetSize::Small);
                let storage = utils::create_storage(&config).await;
                let indexer = Indexer::new(config.clone(), storage).await.unwrap();

                // Initial indexing
                indexer.index_workspaces().await.unwrap();

                // Modify some files
                for i in 0..3 {
                    let file_path = workspace.join(format!("file_{}.rs", i));
                    if let Ok(mut content) = std::fs::read_to_string(&file_path) {
                        content.push_str("\n// Modified for benchmark");
                        let _ = std::fs::write(&file_path, content);
                    }
                }

                // Re-index to detect changes
                let _: () = indexer.index_workspaces().await.unwrap();
                black_box(());
            });
        });
    });

    group.finish();
}

fn benchmark_concurrent_indexing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("indexing/concurrent");

    for thread_count in [1, 2, 4] {
        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let (_temp, _workspace, mut config) =
                            utils::setup_benchmark_workspace(utils::DatasetSize::Small);
                        Arc::get_mut(&mut config).unwrap().indexing_threads = thread_count;

                        let storage = utils::create_storage(&config).await;
                        let indexer = Indexer::new(config.clone(), storage).await.unwrap();

                        let _: () = indexer.index_workspaces().await.unwrap();
                        black_box(());
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_file_indexing,
    benchmark_tantivy_operations,
    benchmark_symbol_extraction,
    benchmark_incremental_indexing,
    benchmark_concurrent_indexing
);
criterion_main!(benches);
