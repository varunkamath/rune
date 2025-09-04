mod utils;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rune_core::{
    indexing::Indexer,
    search::{SearchEngine, SearchMode, SearchQuery},
};
use std::hint::black_box;
use tokio::runtime::Runtime;

fn benchmark_literal_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("search/literal");

    // Setup once for all benchmarks in this group
    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Medium);

    // Do all async setup outside the benchmark loop
    let storage = rt.block_on(async {
        let storage = utils::create_storage(&config).await;
        let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
        indexer.index_workspaces().await.unwrap();
        storage
    });

    for query_str in utils::get_benchmark_queries() {
        group.bench_with_input(
            BenchmarkId::new("query", query_str),
            &query_str,
            |b, &query_str| {
                // Create search engine inside the benchmark loop to avoid clone issues
                b.iter(|| {
                    rt.block_on(async {
                        let search_engine = SearchEngine::new(config.clone(), storage.clone())
                            .await
                            .unwrap();
                        let query = SearchQuery {
                            query: query_str.to_string(),
                            mode: SearchMode::Literal,
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

fn benchmark_regex_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("search/regex");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Medium);

    // Do all async setup outside the benchmark loop
    let storage = rt.block_on(async {
        let storage = utils::create_storage(&config).await;
        let indexer = Indexer::new(config.clone(), storage.clone()).await.unwrap();
        indexer.index_workspaces().await.unwrap();
        storage
    });

    for pattern in utils::get_regex_patterns() {
        group.bench_with_input(
            BenchmarkId::new("pattern", pattern),
            &pattern,
            |b, &pattern| {
                b.iter(|| {
                    rt.block_on(async {
                        let search_engine = SearchEngine::new(config.clone(), storage.clone())
                            .await
                            .unwrap();
                        let query = SearchQuery {
                            query: pattern.to_string(),
                            mode: SearchMode::Regex,
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

fn benchmark_symbol_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("search/symbol");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Medium);

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

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Medium);

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
                        mode: SearchMode::Literal,
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

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Large);

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
                    mode: SearchMode::Literal,
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
    benchmark_literal_search,
    benchmark_regex_search,
    benchmark_symbol_search,
    benchmark_search_with_filters,
    benchmark_search_latency_percentiles
);
criterion_main!(benches);
