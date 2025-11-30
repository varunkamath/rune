mod utils;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rune_core::{
    indexing::Indexer,
    search::{SearchEngine, SearchMode, SearchQuery},
};
use std::hint::black_box;
use tokio::runtime::Runtime;

fn benchmark_symbol_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("search/symbol");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Small);

    // Do all async setup outside the benchmark loop
    let storage = rt.block_on(async {
        let storage = utils::create_storage(&config).await;
        let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
        indexer.index_workspaces().await.unwrap();
        storage
    });

    let symbols = vec!["struct", "impl", "fn", "trait", "enum"];
    for symbol_type in symbols {
        group.bench_with_input(
            BenchmarkId::new("symbol", symbol_type),
            &symbol_type,
            |b, &symbol_type| {
                b.iter(|| {
                    rt.block_on(async {
                        let search_engine = SearchEngine::new(config.clone(), storage.clone())
                            .await
                            .unwrap();
                        let query = SearchQuery {
                            query: symbol_type.to_string(),
                            mode: SearchMode::Symbol,
                            limit: 10,
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

fn benchmark_search_with_filters(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("search/filtered");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Small);

    let storage = rt.block_on(async {
        let storage = utils::create_storage(&config).await;
        let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
        indexer.index_workspaces().await.unwrap();
        storage
    });

    // Test different filter combinations
    let filters = vec![
        ("no_filter", None, None),
        ("with_file_pattern", None, Some(vec!["*.rs".to_string()])),
        ("with_repo", Some(vec!["repo1".to_string()]), None),
        (
            "with_both",
            Some(vec!["repo1".to_string()]),
            Some(vec!["*.rs".to_string()]),
        ),
    ];

    for (filter_name, repos, patterns) in filters {
        group.bench_function(filter_name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    let search_engine = SearchEngine::new(config.clone(), storage.clone())
                        .await
                        .unwrap();
                    let query = SearchQuery {
                        query: "test".to_string(),
                        mode: SearchMode::Symbol,
                        limit: 10,
                        offset: 0,
                        repositories: repos.clone(),
                        file_patterns: patterns.clone(),
                    };

                    black_box(search_engine.search(query).await.unwrap());
                });
            });
        });
    }

    group.finish();
}

fn benchmark_search_latency_percentiles(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("search/latency");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Small);

    let storage = rt.block_on(async {
        let storage = utils::create_storage(&config).await;
        let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
        indexer.index_workspaces().await.unwrap();
        storage
    });

    // Common query to test latency
    group.bench_function("p50_p95_p99", |b| {
        b.iter(|| {
            rt.block_on(async {
                let search_engine = SearchEngine::new(config.clone(), storage.clone())
                    .await
                    .unwrap();
                let query = SearchQuery {
                    query: "function".to_string(),
                    mode: SearchMode::Symbol,
                    limit: 50,
                    offset: 0,
                    repositories: None,
                    file_patterns: None,
                };

                black_box(search_engine.search(query).await.unwrap());
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_symbol_search,
    benchmark_search_with_filters,
    benchmark_search_latency_percentiles
);
criterion_main!(benches);
