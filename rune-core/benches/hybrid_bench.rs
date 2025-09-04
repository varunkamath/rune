mod utils;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rune_core::{
    indexing::Indexer,
    search::{SearchEngine, SearchMode, SearchQuery},
};
use std::hint::black_box;
use tokio::runtime::Runtime;

fn benchmark_rrf_fusion(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("hybrid/rrf");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Medium);

    // Setup outside of benchmark loop to avoid nested runtime
    let storage = rt.block_on(async {
        let storage = utils::create_storage(&config).await;
        let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
        indexer.index_workspaces().await.unwrap();
        storage
    });

    // Benchmark hybrid search with different query complexities
    let queries = vec![
        ("simple", "function"),
        ("medium", "calculate error"),
        ("complex", "impl trait for struct"),
    ];

    for (complexity, query_str) in queries {
        group.bench_with_input(
            BenchmarkId::new("query_complexity", complexity),
            &query_str,
            |b, query_str| {
                // Create search engine per iteration to avoid clone issues
                b.iter(|| {
                    rt.block_on(async {
                        let search_engine = SearchEngine::new(config.clone(), storage.clone())
                            .await
                            .unwrap();
                        let query = SearchQuery {
                            query: query_str.to_string(),
                            mode: SearchMode::Hybrid,
                            limit: 20,
                            offset: 0,
                            repositories: None,
                            file_patterns: None,
                        };

                        black_box(search_engine.search(query).await.unwrap());
                    });
                });
            },
        );
    }

    group.finish();
}

fn benchmark_result_merging(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("hybrid/merging");

    let sizes = vec![
        (utils::DatasetSize::Small, "small"),
        (utils::DatasetSize::Medium, "medium"),
    ];

    for (size, size_name) in sizes {
        group.bench_with_input(
            BenchmarkId::new("dataset_size", size_name),
            &size,
            |b, size| {
                // Setup benchmark data once per size
                let (_temp, _workspace, config) = utils::setup_benchmark_workspace(*size);
                let storage = rt.block_on(async {
                    let storage = utils::create_storage(&config).await;
                    let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
                    indexer.index_workspaces().await.unwrap();
                    storage
                });

                b.iter(|| {
                    rt.block_on(async {
                        let search_engine = SearchEngine::new(config.clone(), storage.clone())
                            .await
                            .unwrap();

                        let query = SearchQuery {
                            query: "return".to_string(), // Common keyword for more results
                            mode: SearchMode::Hybrid,
                            limit: 50,
                            offset: 0,
                            repositories: None,
                            file_patterns: None,
                        };

                        black_box(search_engine.search(query).await.unwrap());
                    });
                });
            },
        );
    }

    group.finish();
}

fn benchmark_hybrid_vs_individual_modes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("hybrid/comparison");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Medium);

    // Setup outside of benchmark loop
    let storage = rt.block_on(async {
        let storage = utils::create_storage(&config).await;
        let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
        indexer.index_workspaces().await.unwrap();
        storage
    });

    let query_str = "calculate";

    // Benchmark each mode
    for mode in [
        SearchMode::Literal,
        SearchMode::Regex,
        SearchMode::Symbol,
        SearchMode::Hybrid,
    ] {
        let mode_name = match mode {
            SearchMode::Literal => "literal",
            SearchMode::Regex => "regex",
            SearchMode::Symbol => "symbol",
            SearchMode::Hybrid => "hybrid",
            _ => "unknown",
        };

        group.bench_with_input(BenchmarkId::new("mode", mode_name), &mode, |b, mode| {
            b.iter(|| {
                rt.block_on(async {
                    let search_engine = SearchEngine::new(config.clone(), storage.clone())
                        .await
                        .unwrap();
                    let query = SearchQuery {
                        query: query_str.to_string(),
                        mode: mode.clone(),
                        limit: 20,
                        offset: 0,
                        repositories: None,
                        file_patterns: None,
                    };

                    black_box(search_engine.search(query).await.unwrap());
                });
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_rrf_fusion,
    benchmark_result_merging,
    benchmark_hybrid_vs_individual_modes
);
criterion_main!(benches);
