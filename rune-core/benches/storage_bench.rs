mod utils;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rune_core::storage::FileMetadata;
use std::hint::black_box;
use std::path::PathBuf;
use tokio::runtime::Runtime;

fn benchmark_storage_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/write");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Small);

    // Create runtime and storage outside the benchmark loop
    let rt = Runtime::new().unwrap();
    let storage = rt.block_on(async { utils::create_storage(&config).await });

    // Benchmark different metadata sizes
    let metadata_samples = vec![
        ("small", FileMetadata {
            path: PathBuf::from("test.rs"),
            size: 100,
            modified: 1234567890,
            language: "rust".to_string(),
            hash: "abc123".to_string(),
            indexed_at: 1234567890,
        }),
        ("medium", FileMetadata {
            path: PathBuf::from("very/long/path/to/some/file/test.rs"),
            size: 10000,
            modified: 1234567890,
            language: "rust".to_string(),
            hash: "abc123def456ghi789jkl012mno345pqr678stu901vwx234yz".to_string(), // pragma: allowlist secret
            indexed_at: 1234567890,
        }),
        ("large", FileMetadata {
            path: PathBuf::from("extremely/long/path/with/many/nested/directories/and/a/very/long/filename/that/goes/on/and/on/test.rs"),
            size: 1000000,
            modified: 1234567890,
            language: "rust".to_string(),
            hash: "very_long_hash_value_that_contains_lots_of_characters_to_test_storage_performance_with_larger_payloads".to_string(),
            indexed_at: 1234567890,
        }),
    ];

    for (size_name, metadata) in metadata_samples {
        group.bench_with_input(
            BenchmarkId::new("metadata_size", size_name),
            &metadata,
            |b, metadata| {
                let mut counter = 0;
                b.iter(|| {
                    let path = PathBuf::from(format!("bench_file_{}.rs", counter));
                    counter += 1;
                    let result = rt.block_on(storage.store_file_metadata(&path, metadata.clone()));
                    let _ = black_box(result);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_storage_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/read");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Small);

    let rt = Runtime::new().unwrap();
    let storage = rt.block_on(async {
        let storage = utils::create_storage(&config).await;

        // Pre-populate storage with test data
        for i in 0..100 {
            let path = PathBuf::from(format!("test_file_{}.rs", i));
            let metadata = FileMetadata {
                path: path.clone(),
                size: 1000,
                modified: 1234567890,
                language: "rust".to_string(),
                hash: format!("hash_{}", i),
                indexed_at: 1234567890,
            };
            storage.store_file_metadata(&path, metadata).await.unwrap();
        }

        storage
    });

    // Benchmark sequential reads
    group.bench_function("sequential", |b| {
        let mut counter = 0;
        b.iter(|| {
            let path = PathBuf::from(format!("test_file_{}.rs", counter % 100));
            counter += 1;
            let result = rt.block_on(storage.get_file_metadata(&path));
            let _ = black_box(result);
        });
    });

    // Benchmark random reads
    group.bench_function("random", |b| {
        b.iter(|| {
            let idx = rand::random::<usize>() % 100;
            let path = PathBuf::from(format!("test_file_{}.rs", idx));
            let result = rt.block_on(storage.get_file_metadata(&path));
            let _ = black_box(result);
        });
    });

    group.finish();
}

fn benchmark_storage_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/batch");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Small);

    let rt = Runtime::new().unwrap();
    let storage = rt.block_on(async { utils::create_storage(&config).await });

    // Benchmark batch writes
    for batch_size in [10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("batch_write", batch_size),
            &batch_size,
            |b, &batch_size| {
                let mut counter = 0;
                b.iter(|| {
                    let futures: Vec<_> = (0..batch_size)
                        .map(|i| {
                            let path = PathBuf::from(format!("batch_{}_{}.rs", counter, i));
                            let metadata = FileMetadata {
                                path: path.clone(),
                                size: 1000,
                                modified: 1234567890,
                                language: "rust".to_string(),
                                hash: format!("hash_{}_{}", counter, i),
                                indexed_at: 1234567890,
                            };
                            let storage_clone = storage.clone();
                            async move { storage_clone.store_file_metadata(&path, metadata).await }
                        })
                        .collect();

                    counter += 1;
                    let results = rt.block_on(futures::future::join_all(futures));
                    black_box(results);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_storage_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/list");

    for dataset_size in [utils::DatasetSize::Small, utils::DatasetSize::Medium] {
        let (_temp, _workspace, config) = utils::setup_benchmark_workspace(dataset_size);

        let rt = Runtime::new().unwrap();
        let storage = rt.block_on(async {
            let storage = utils::create_storage(&config).await;

            // Pre-populate storage based on dataset size
            let num_files = match dataset_size {
                utils::DatasetSize::Small => 10,
                utils::DatasetSize::Medium => 100,
                utils::DatasetSize::Large => 1000,
            };

            for i in 0..num_files {
                let path = PathBuf::from(format!("file_{}.rs", i));
                let metadata = FileMetadata {
                    path: path.clone(),
                    size: 1000,
                    modified: 1234567890,
                    language: "rust".to_string(),
                    hash: format!("hash_{}", i),
                    indexed_at: 1234567890,
                };
                storage.store_file_metadata(&path, metadata).await.unwrap();
            }

            storage
        });

        group.bench_with_input(
            BenchmarkId::new("list_files", format!("{:?}", dataset_size)),
            &(),
            |b, _| {
                b.iter(|| {
                    let result = rt.block_on(storage.list_files());
                    let _ = black_box(result);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_storage_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/stats");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Medium);

    let rt = Runtime::new().unwrap();
    let storage = rt.block_on(async {
        let storage = utils::create_storage(&config).await;

        // Pre-populate storage
        for i in 0..100 {
            let path = PathBuf::from(format!("test_file_{}.rs", i));
            let metadata = FileMetadata {
                path: path.clone(),
                size: 1000 + i as u64,
                modified: 1234567890,
                language: "rust".to_string(),
                hash: format!("hash_{}", i),
                indexed_at: 1234567890,
            };
            storage.store_file_metadata(&path, metadata).await.unwrap();
        }

        storage
    });

    group.bench_function("get_file_count", |b| {
        b.iter(|| {
            let result = rt.block_on(storage.get_file_count());
            let _ = black_box(result);
        });
    });

    group.bench_function("get_symbol_count", |b| {
        b.iter(|| {
            let result = rt.block_on(storage.get_symbol_count());
            let _ = black_box(result);
        });
    });

    group.bench_function("get_cache_size", |b| {
        b.iter(|| {
            let result = rt.block_on(storage.get_cache_size());
            let _ = black_box(result);
        });
    });

    group.finish();
}

fn benchmark_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/concurrent");

    let (_temp, _workspace, config) = utils::setup_benchmark_workspace(utils::DatasetSize::Small);

    let rt = Runtime::new().unwrap();
    let storage = rt.block_on(async {
        let storage = utils::create_storage(&config).await;

        // Pre-populate storage
        for i in 0..50 {
            let path = PathBuf::from(format!("concurrent_file_{}.rs", i));
            let metadata = FileMetadata {
                path: path.clone(),
                size: 1000,
                modified: 1234567890,
                language: "rust".to_string(),
                hash: format!("hash_{}", i),
                indexed_at: 1234567890,
            };
            storage.store_file_metadata(&path, metadata).await.unwrap();
        }

        storage
    });

    group.bench_function("mixed_read_write", |b| {
        b.iter(|| {
            use futures::future::join_all;

            let mut read_futures = vec![];
            let mut write_futures = vec![];

            // Spawn 5 readers
            for i in 0..5 {
                let storage_clone = storage.clone();
                let future = async move {
                    let path = PathBuf::from(format!("concurrent_file_{}.rs", i * 10));
                    storage_clone.get_file_metadata(&path).await
                };
                read_futures.push(future);
            }

            // Spawn 5 writers
            for i in 0..5 {
                let storage_clone = storage.clone();
                let future = async move {
                    let path = PathBuf::from(format!("new_file_{}.rs", i));
                    let metadata = FileMetadata {
                        path: path.clone(),
                        size: 2000,
                        modified: 1234567891,
                        language: "rust".to_string(),
                        hash: format!("new_hash_{}", i),
                        indexed_at: 1234567891,
                    };
                    storage_clone.store_file_metadata(&path, metadata).await
                };
                write_futures.push(future);
            }

            // Wait for all tasks
            let read_results = rt.block_on(join_all(read_futures));
            let write_results = rt.block_on(join_all(write_futures));
            black_box((read_results, write_results));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_storage_write,
    benchmark_storage_read,
    benchmark_storage_batch,
    benchmark_storage_list,
    benchmark_storage_stats,
    benchmark_concurrent_access
);

criterion_main!(benches);
