use rune_core::{Config, storage::StorageBackend};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

/// Standard benchmark file sizes
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum DatasetSize {
    Small,  // 10 files
    Medium, // 100 files
    Large,  // 1000 files
}

impl DatasetSize {
    pub fn file_count(&self) -> usize {
        match self {
            DatasetSize::Small => 10,
            DatasetSize::Medium => 100,
            DatasetSize::Large => 1000,
        }
    }
}

/// Generate sample code files for benchmarking
pub fn generate_sample_files(size: DatasetSize) -> Vec<(String, String)> {
    let count = size.file_count();
    let mut files = Vec::with_capacity(count);

    // Templates for different languages
    let rust_template = include_str!("../../test_workspace/data_structures.rs");
    let python_template = include_str!("../../test_workspace/math_operations.py");
    let js_template = include_str!("../../test_workspace/string_utils.js");
    let go_template = include_str!("../../test_workspace/file_operations.go");

    for i in 0..count {
        let (name, content) = match i % 4 {
            0 => (format!("file_{}.rs", i), modify_template(rust_template, i)),
            1 => (
                format!("file_{}.py", i),
                modify_template(python_template, i),
            ),
            2 => (format!("file_{}.js", i), modify_template(js_template, i)),
            _ => (format!("file_{}.go", i), modify_template(go_template, i)),
        };
        files.push((name, content));
    }

    files
}

/// Modify template to create unique content
fn modify_template(template: &str, index: usize) -> String {
    // Add unique markers to make each file slightly different
    format!(
        "// Benchmark file #{}\n{}\n// End of file #{}",
        index, template, index
    )
}

/// Create a benchmark workspace with generated files
pub fn setup_benchmark_workspace(size: DatasetSize) -> (TempDir, PathBuf, Arc<Config>) {
    let temp_dir = tempfile::tempdir().unwrap();
    let workspace = temp_dir.path().join("workspace");
    fs::create_dir(&workspace).unwrap();

    // Generate and write files
    let files = generate_sample_files(size);
    for (name, content) in files {
        fs::write(workspace.join(&name), &content).unwrap();
    }

    let config = Arc::new(Config {
        workspace_roots: vec![workspace.clone()],
        workspace_dir: workspace.to_string_lossy().to_string(),
        cache_dir: temp_dir.path().join("cache"),
        max_file_size: 10 * 1024 * 1024,
        indexing_threads: num_cpus::get(),
        enable_semantic: false, // Disable by default for benchmarks
        languages: vec![
            "rust".to_string(),
            "python".to_string(),
            "javascript".to_string(),
            "typescript".to_string(),
            "go".to_string(),
        ],
    });

    (temp_dir, workspace, config)
}

/// Create a storage backend for benchmarks
pub async fn create_storage(config: &Config) -> StorageBackend {
    StorageBackend::new(&config.cache_dir).await.unwrap()
}

/// Common search queries for benchmarking
#[allow(dead_code)]
pub fn get_benchmark_queries() -> Vec<&'static str> {
    vec![
        "function",     // Common keyword
        "calculate",    // Specific function name
        "impl",         // Language-specific keyword
        "def",          // Python keyword
        "const",        // JavaScript keyword
        "error",        // Common in all languages
        "return",       // Universal keyword
        "test",         // Common in test files
        "TODO",         // Comment marker
        "struct Point", // Multi-word query
    ]
}

/// Complex regex patterns for benchmarking
#[allow(dead_code)]
pub fn get_regex_patterns() -> Vec<&'static str> {
    vec![
        r"\bfn\s+\w+",             // Rust function
        r"def\s+\w+\(",            // Python function
        r"class\s+\w+",            // Class definition
        r"//.*TODO",               // TODO comments
        r"\b[A-Z][a-z]+[A-Z]\w*",  // CamelCase
        r"\w+Error",               // Error types
        r"test_\w+",               // Test functions
        r"\d{3,}",                 // Numbers with 3+ digits
        r"impl\s+\w+\s+for\s+\w+", // Rust impl blocks
        r"async\s+\w+",            // Async functions
    ]
}
