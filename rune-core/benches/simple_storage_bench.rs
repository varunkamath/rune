use criterion::{Criterion, criterion_group, criterion_main};
use rune_core::storage::{FileMetadata, StorageBackend};
use std::hint::black_box;
use std::path::PathBuf;

fn simple_storage_benchmark(c: &mut Criterion) {
    c.bench_function("storage_write", |b| {
        let temp_dir = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();

        b.iter(|| {
            let storage =
                rt.block_on(async { StorageBackend::new(temp_dir.path()).await.unwrap() });

            let metadata = FileMetadata {
                path: PathBuf::from("test.rs"),
                size: 100,
                modified: 1234567890,
                language: "rust".to_string(),
                hash: "abc123".to_string(),
                indexed_at: 1234567890,
            };

            rt.block_on(async {
                storage
                    .store_file_metadata(&PathBuf::from("test.rs"), metadata)
                    .await
            })
            .unwrap();

            black_box(storage);
        });
    });
}

criterion_group!(benches, simple_storage_benchmark);
criterion_main!(benches);
