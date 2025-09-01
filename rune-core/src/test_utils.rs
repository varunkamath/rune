// Re-export for easier use in tests
#[cfg(test)]
pub use self::test_helpers::*;

#[cfg(test)]
pub mod test_helpers {
    use crate::{
        Config,
        indexing::{Indexer, tantivy_indexer::TantivyIndexer},
        search::{SearchMode, SearchQuery, SearchResult},
        storage::StorageBackend,
    };
    use anyhow::Result;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::{TempDir, tempdir};

    /// Test file structure for creating test data
    pub struct TestFile {
        pub name: String,
        pub content: String,
    }

    /// Create a test workspace with the given files
    pub async fn create_test_workspace(
        files: Vec<TestFile>,
    ) -> Result<(TempDir, PathBuf, Arc<Config>)> {
        let temp_dir = tempdir()?;
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace)?;

        // Write all test files
        for file in files {
            fs::write(workspace.join(&file.name), &file.content)?;
        }

        let config = Arc::new(Config {
            workspace_roots: vec![workspace.clone()],
            workspace_dir: workspace.to_string_lossy().to_string(),
            cache_dir: temp_dir.path().join("cache"),
            max_file_size: 10 * 1024 * 1024,
            indexing_threads: 2,
            enable_semantic: false, // Disable semantic for basic tests
            languages: vec![
                "rust".to_string(),
                "python".to_string(),
                "javascript".to_string(),
                "typescript".to_string(),
                "go".to_string(),
            ],
        });

        Ok((temp_dir, workspace, config))
    }

    /// Create and index a test workspace, returning everything needed for search tests
    pub async fn setup_indexed_workspace(
        files: Vec<TestFile>,
    ) -> Result<(TempDir, Arc<Config>, StorageBackend, Arc<TantivyIndexer>)> {
        let (temp_dir, _workspace, config) = create_test_workspace(files).await?;

        // Create storage backend
        let storage = StorageBackend::new(&config.cache_dir).await?;

        // Create and initialize indexer
        let indexer = Indexer::new(config.clone(), storage.clone()).await?;

        // Index all files in the workspace
        indexer.index_workspaces().await?;

        // Drop the indexer to release write lock
        drop(indexer);

        // Create a new Tantivy indexer for searching (read-only)
        let tantivy_indexer =
            Arc::new(TantivyIndexer::new_read_only(&config.cache_dir.join("tantivy_index")).await?);

        Ok((temp_dir, config, storage, tantivy_indexer))
    }

    /// Common test files for search tests
    pub fn get_standard_test_files() -> Vec<TestFile> {
        vec![
            TestFile {
                name: "calculator.rs".to_string(),
                content: r#"
use std::collections::HashMap;

/// A simple calculator struct
pub struct Calculator {
    memory: f64,
}

impl Calculator {
    pub fn new() -> Self {
        Self { memory: 0.0 }
    }
    
    pub fn calculate_sum(a: f64, b: f64) -> f64 {
        a + b
    }
    
    pub fn calculate_product(a: f64, b: f64) -> f64 {
        a * b
    }
    
    pub fn calculate_difference(a: f64, b: f64) -> f64 {
        a - b
    }
}

fn main() {
    let calc = Calculator::new();
    let result = Calculator::calculate_sum(10.0, 20.0);
    println!("Sum: {}", result);
}
"#
                .to_string(),
            },
            TestFile {
                name: "math_utils.py".to_string(),
                content: r#"
"""Mathematical utility functions"""

def calculate_sum(a, b):
    """Calculate the sum of two numbers"""
    return a + b

def calculate_product(a, b):
    """Calculate the product of two numbers"""
    return a * b

def calculate_average(numbers):
    """Calculate the average of a list of numbers"""
    if not numbers:
        return 0
    return sum(numbers) / len(numbers)

class MathOperations:
    def __init__(self):
        self.history = []
    
    def add(self, a, b):
        result = a + b
        self.history.append(result)
        return result
    
    def multiply(self, a, b):
        result = a * b
        self.history.append(result)
        return result

# Global calculator instance
calculator = MathOperations()
"#
                .to_string(),
            },
            TestFile {
                name: "utils.js".to_string(),
                content: r#"
// JavaScript utility functions

function calculateSum(a, b) {
    return a + b;
}

const calculateProduct = (a, b) => {
    return a * b;
};

class Calculator {
    constructor() {
        this.memory = 0;
    }
    
    calculate(operation, a, b) {
        switch(operation) {
            case 'sum':
                return a + b;
            case 'product':
                return a * b;
            case 'difference':
                return a - b;
            default:
                throw new Error('Unknown operation');
        }
    }
}

// Export functions
module.exports = {
    calculateSum,
    calculateProduct,
    Calculator
};
"#
                .to_string(),
            },
        ]
    }

    /// Helper to create test files with patterns for regex testing
    pub fn get_regex_test_files() -> Vec<TestFile> {
        vec![
            TestFile {
                name: "patterns.rs".to_string(),
                content: r#"
fn process_data1() {
    let data1 = vec![1, 2, 3];
    let data2 = vec![4, 5, 6];
    let data3 = vec![7, 8, 9];
}

fn handle_error_001() {
    println!("Error 001 occurred");
}

fn handle_error_002() {
    println!("Error 002 occurred");
}

fn handle_error_123() {
    println!("Error 123 occurred");
}

// Variables with pattern
let user_123 = "John";
let user_456 = "Jane";
let admin_789 = "Admin";
"#
                .to_string(),
            },
            TestFile {
                name: "data.py".to_string(),
                content: r#"
import re

# Email patterns
email1 = "user@example.com"
email2 = "admin@test.org"
email3 = "support@company.net"

# Phone patterns  
phone1 = "123-456-7890"
phone2 = "987-654-3210"
phone3 = "555-123-4567"

# IP addresses
ip1 = "192.168.1.1"
ip2 = "10.0.0.1"
ip3 = "172.16.0.1"

# Dates
date1 = "2024-01-15"
date2 = "2024-12-31"
date3 = "2023-06-30"
"#
                .to_string(),
            },
        ]
    }

    /// Assert that search results contain expected content
    pub fn assert_results_contain(results: &[SearchResult], expected_content: &[&str]) {
        for content in expected_content {
            assert!(
                results.iter().any(|r| r.content.contains(content)),
                "Results should contain '{}'",
                content
            );
        }
    }

    /// Assert that all results are from specific file extensions
    pub fn assert_results_file_type(results: &[SearchResult], extension: &str) {
        for result in results {
            assert_eq!(
                result.file_path.extension().and_then(|e| e.to_str()),
                Some(extension),
                "All results should be from .{} files",
                extension
            );
        }
    }

    /// Create a basic search query for testing
    pub fn create_test_query(query: &str, mode: SearchMode) -> SearchQuery {
        SearchQuery {
            query: query.to_string(),
            mode,
            limit: 50,
            offset: 0,
            repositories: None,
            file_patterns: None,
        }
    }
}
