use rune_core::{Config, RuneEngine};
use std::path::PathBuf;
use tempfile::tempdir;

/// Helper to check if Qdrant is available
async fn is_qdrant_available() -> bool {
    match reqwest::Client::new()
        .get("http://127.0.0.1:6333/health")
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Create a test workspace with sample files
fn create_test_workspace() -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let path = dir.path().to_path_buf();

    // Create sample Rust file
    let rust_code = r#"
use std::collections::HashMap;

/// A simple key-value cache implementation
pub struct Cache<K, V> {
    data: HashMap<K, V>,
    max_size: usize,
}

impl<K, V> Cache<K, V>
where
    K: std::hash::Hash + Eq,
    V: Clone,
{
    pub fn new(max_size: usize) -> Self {
        Self {
            data: HashMap::new(),
            max_size,
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.data.get(key)
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.data.len() >= self.max_size {
            // Simple eviction: remove first item
            if let Some(first_key) = self.data.keys().next().cloned() {
                self.data.remove(&first_key);
            }
        }
        self.data.insert(key, value);
    }
}
"#;
    std::fs::write(path.join("cache.rs"), rust_code).unwrap();

    // Create sample Python file
    let python_code = r#"
import hashlib
import bcrypt

class AuthenticationService:
    """Handles user authentication and password management"""

    def __init__(self):
        self.users = {}
        self.sessions = {}

    def hash_password(self, password: str) -> str:
        """Hash a password using bcrypt"""
        salt = bcrypt.gensalt()
        hashed = bcrypt.hashpw(password.encode('utf-8'), salt)
        return hashed.decode('utf-8')

    def verify_password(self, password: str, hashed: str) -> bool:
        """Verify a password against its hash"""
        return bcrypt.checkpw(password.encode('utf-8'), hashed.encode('utf-8'))

    def create_user(self, username: str, password: str):
        """Create a new user with hashed password"""
        if username in self.users:
            raise ValueError("User already exists")

        self.users[username] = {
            'password_hash': self.hash_password(password),
            'created_at': datetime.now()
        }

    def authenticate(self, username: str, password: str) -> bool:
        """Authenticate a user"""
        if username not in self.users:
            return False

        user = self.users[username]
        return self.verify_password(password, user['password_hash'])
"#;
    std::fs::write(path.join("auth.py"), python_code).unwrap();

    // Create sample JavaScript file
    let js_code = r#"
// Database connection pool manager
class DatabasePool {
    constructor(config) {
        this.config = config;
        this.connections = [];
        this.maxConnections = config.maxConnections || 10;
        this.activeConnections = 0;
    }

    async getConnection() {
        // Check if we have available connections
        if (this.connections.length > 0) {
            return this.connections.pop();
        }

        // Create new connection if under limit
        if (this.activeConnections < this.maxConnections) {
            const connection = await this.createConnection();
            this.activeConnections++;
            return connection;
        }

        // Wait for a connection to become available
        return new Promise((resolve) => {
            const checkInterval = setInterval(() => {
                if (this.connections.length > 0) {
                    clearInterval(checkInterval);
                    resolve(this.connections.pop());
                }
            }, 100);
        });
    }

    async createConnection() {
        // Simulate database connection
        return {
            query: async (sql, params) => {
                // Execute query
                console.log(`Executing: ${sql}`);
                return { rows: [], affected: 0 };
            },
            close: () => {
                this.activeConnections--;
            }
        };
    }

    releaseConnection(connection) {
        if (this.connections.length < this.maxConnections) {
            this.connections.push(connection);
        } else {
            connection.close();
        }
    }
}

module.exports = DatabasePool;
"#;
    std::fs::write(path.join("database_pool.js"), js_code).unwrap();

    // Create sample Go file
    let go_code = r#"
package main

import (
    "fmt"
    "net/http"
    "time"
)

// HTTPClient with retry logic and circuit breaker
type HTTPClient struct {
    client      *http.Client
    maxRetries  int
    retryDelay  time.Duration
    failures    int
    threshold   int
    resetAfter  time.Duration
    lastFailure time.Time
    circuitOpen bool
}

// NewHTTPClient creates a new HTTP client with retry capabilities
func NewHTTPClient(maxRetries int, threshold int) *HTTPClient {
    return &HTTPClient{
        client: &http.Client{
            Timeout: 30 * time.Second,
        },
        maxRetries:  maxRetries,
        retryDelay:  time.Second,
        threshold:   threshold,
        resetAfter:  60 * time.Second,
        circuitOpen: false,
    }
}

// Get performs an HTTP GET request with retry logic
func (c *HTTPClient) Get(url string) (*http.Response, error) {
    if c.circuitOpen {
        if time.Since(c.lastFailure) < c.resetAfter {
            return nil, fmt.Errorf("circuit breaker is open")
        }
        c.circuitOpen = false
        c.failures = 0
    }

    var lastErr error
    for i := 0; i <= c.maxRetries; i++ {
        if i > 0 {
            time.Sleep(c.retryDelay * time.Duration(i))
        }

        resp, err := c.client.Get(url)
        if err == nil && resp.StatusCode < 500 {
            c.failures = 0
            return resp, nil
        }

        lastErr = err
        c.failures++

        if c.failures >= c.threshold {
            c.circuitOpen = true
            c.lastFailure = time.Now()
            break
        }
    }

    return nil, lastErr
}
"#;
    std::fs::write(path.join("http_client.go"), go_code).unwrap();

    (dir, path)
}

#[tokio::test]
async fn test_semantic_search_with_real_qdrant() {
    // Skip test if Qdrant is not available
    if !is_qdrant_available().await {
        eprintln!("Skipping test: Qdrant is not running on localhost:6333");
        return;
    }

    // Create test workspace
    let (_temp_dir, workspace_path) = create_test_workspace();

    // Create config with semantic enabled
    let config = Config {
        workspace_roots: vec![workspace_path.clone()],
        workspace_dir: workspace_path.to_string_lossy().to_string(),
        cache_dir: tempdir().unwrap().path().to_path_buf(),
        max_file_size: 10 * 1024 * 1024,
        indexing_threads: 2,
        enable_semantic: true,
        languages: vec![
            "rust".to_string(),
            "python".to_string(),
            "javascript".to_string(),
            "go".to_string(),
        ],
        file_watch_debounce_ms: 500,
    };

    // Set environment variable
    unsafe {
        std::env::set_var("RUNE_ENABLE_SEMANTIC", "true");
        std::env::set_var("QDRANT_URL", "http://127.0.0.1:6334");
    }

    // Initialize engine
    let mut engine = RuneEngine::new(config).await.unwrap();
    engine.start().await.unwrap();

    // Wait for indexing to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Test 1: Search for caching concepts
    let cache_query = rune_core::search::SearchQuery {
        query: "cache implementation memory storage".to_string(),
        mode: rune_core::search::SearchMode::Semantic,
        repositories: None,
        file_patterns: None,
        limit: 5,
        offset: 0,
    };

    let cache_results = engine.search().search(cache_query).await.unwrap();
    assert!(
        cache_results.total_matches > 0,
        "Should find cache-related results"
    );

    // The cache.rs file should be in the results
    let has_cache_file = cache_results
        .results
        .iter()
        .any(|r| r.file_path.to_string_lossy().contains("cache.rs"));
    assert!(has_cache_file, "Should find cache.rs file");

    // Test 2: Search for authentication concepts
    let auth_query = rune_core::search::SearchQuery {
        query: "password hashing authentication security".to_string(),
        mode: rune_core::search::SearchMode::Semantic,
        repositories: None,
        file_patterns: None,
        limit: 5,
        offset: 0,
    };

    let auth_results = engine.search().search(auth_query).await.unwrap();
    assert!(
        auth_results.total_matches > 0,
        "Should find auth-related results"
    );

    // The auth.py file should be in the results
    let has_auth_file = auth_results
        .results
        .iter()
        .any(|r| r.file_path.to_string_lossy().contains("auth.py"));
    assert!(has_auth_file, "Should find auth.py file");

    // Test 3: Search for database concepts
    let db_query = rune_core::search::SearchQuery {
        query: "database connection pool management".to_string(),
        mode: rune_core::search::SearchMode::Semantic,
        repositories: None,
        file_patterns: None,
        limit: 5,
        offset: 0,
    };

    let db_results = engine.search().search(db_query).await.unwrap();
    assert!(
        db_results.total_matches > 0,
        "Should find database-related results"
    );

    // Test 4: Search for HTTP/networking concepts
    let http_query = rune_core::search::SearchQuery {
        query: "HTTP client retry circuit breaker".to_string(),
        mode: rune_core::search::SearchMode::Semantic,
        repositories: None,
        file_patterns: None,
        limit: 5,
        offset: 0,
    };

    let http_results = engine.search().search(http_query).await.unwrap();
    assert!(
        http_results.total_matches > 0,
        "Should find HTTP-related results"
    );

    // Test 5: Cross-language semantic search
    let general_query = rune_core::search::SearchQuery {
        query: "error handling and retry logic".to_string(),
        mode: rune_core::search::SearchMode::Semantic,
        repositories: None,
        file_patterns: None,
        limit: 10,
        offset: 0,
    };

    let general_results = engine.search().search(general_query).await.unwrap();
    assert!(
        general_results.total_matches > 0,
        "Should find error handling results"
    );

    // Test 6: File pattern filtering with semantic search
    let rust_only_query = rune_core::search::SearchQuery {
        query: "data structures and storage".to_string(),
        mode: rune_core::search::SearchMode::Semantic,
        repositories: None,
        file_patterns: Some(vec!["*.rs".to_string()]),
        limit: 5,
        offset: 0,
    };

    let rust_results = engine.search().search(rust_only_query).await.unwrap();

    // All results should be Rust files
    for result in &rust_results.results {
        assert!(
            result.file_path.to_string_lossy().ends_with(".rs"),
            "Should only return Rust files"
        );
    }

    // Clean up
    unsafe {
        std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        std::env::remove_var("QDRANT_URL");
    }
}

#[tokio::test]
async fn test_semantic_search_without_qdrant() {
    // Create test workspace
    let (_temp_dir, workspace_path) = create_test_workspace();

    // Create config with semantic disabled to ensure no Qdrant connection
    let config = Config {
        workspace_roots: vec![workspace_path.clone()],
        workspace_dir: workspace_path.to_string_lossy().to_string(),
        cache_dir: tempdir().unwrap().path().to_path_buf(),
        max_file_size: 10 * 1024 * 1024,
        indexing_threads: 1,
        enable_semantic: false, // Disable semantic to avoid Qdrant
        languages: vec!["rust".to_string()],
        file_watch_debounce_ms: 500,
    };

    // Also set environment to disable semantic and use bad URL
    unsafe {
        std::env::set_var("RUNE_ENABLE_SEMANTIC", "false");
        std::env::set_var("QDRANT_URL", "http://127.0.0.1:99999");
    }

    // Initialize engine - should work without semantic search
    let mut engine = RuneEngine::new(config).await.unwrap();
    engine.start().await.unwrap();

    // Semantic search should return empty results
    let query = rune_core::search::SearchQuery {
        query: "test query".to_string(),
        mode: rune_core::search::SearchMode::Semantic,
        repositories: None,
        file_patterns: None,
        limit: 5,
        offset: 0,
    };

    let results = engine.search().search(query).await.unwrap();
    assert_eq!(
        results.total_matches, 0,
        "Should return 0 results when semantic is disabled"
    );
    assert_eq!(results.results.len(), 0, "Results should be empty");

    // Clean up
    unsafe {
        std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        std::env::remove_var("QDRANT_URL");
    }
}

#[tokio::test]
async fn test_hybrid_search_mode() {
    // Skip test if Qdrant is not available
    if !is_qdrant_available().await {
        eprintln!("Skipping test: Qdrant is not running on localhost:6333");
        return;
    }

    // Create test workspace
    let (_temp_dir, workspace_path) = create_test_workspace();

    // Create config
    let config = Config {
        workspace_roots: vec![workspace_path.clone()],
        workspace_dir: workspace_path.to_string_lossy().to_string(),
        cache_dir: tempdir().unwrap().path().to_path_buf(),
        max_file_size: 10 * 1024 * 1024,
        indexing_threads: 2,
        enable_semantic: true,
        languages: vec![
            "rust".to_string(),
            "python".to_string(),
            "javascript".to_string(),
            "go".to_string(),
        ],
        file_watch_debounce_ms: 500,
    };

    unsafe {
        std::env::set_var("RUNE_ENABLE_SEMANTIC", "true");
        std::env::set_var("QDRANT_URL", "http://127.0.0.1:6334");
    }

    // Initialize engine
    let mut engine = RuneEngine::new(config).await.unwrap();
    engine.start().await.unwrap();

    // Wait for indexing
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Test hybrid search (combines literal and semantic)
    let hybrid_query = rune_core::search::SearchQuery {
        query: "HashMap".to_string(), // Literal match in cache.rs
        mode: rune_core::search::SearchMode::Hybrid,
        repositories: None,
        file_patterns: None,
        limit: 10,
        offset: 0,
    };

    let hybrid_results = engine.search().search(hybrid_query).await.unwrap();
    assert!(
        hybrid_results.total_matches > 0,
        "Hybrid search should find results"
    );

    // Should find both literal matches and semantically similar code
    let has_literal_match = hybrid_results
        .results
        .iter()
        .any(|r| r.content.contains("HashMap"));
    assert!(has_literal_match, "Should find literal HashMap matches");

    // Clean up
    unsafe {
        std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        std::env::remove_var("QDRANT_URL");
    }
}

#[tokio::test]
async fn test_semantic_search_pagination() {
    // Skip test if Qdrant is not available
    if !is_qdrant_available().await {
        eprintln!("Skipping test: Qdrant is not running on localhost:6333");
        return;
    }

    // Create test workspace with more files
    let (_temp_dir, workspace_path) = create_test_workspace();

    // Add more files for pagination testing
    for i in 0..10 {
        let code = format!(
            r#"
            // Function number {i}
            fn process_data_{i}(input: &str) -> String {{
                // Process and transform the data
                input.to_uppercase()
            }}
            "#
        );
        std::fs::write(workspace_path.join(format!("func_{i}.rs")), code).unwrap();
    }

    let config = Config {
        workspace_roots: vec![workspace_path.clone()],
        workspace_dir: workspace_path.to_string_lossy().to_string(),
        cache_dir: tempdir().unwrap().path().to_path_buf(),
        max_file_size: 10 * 1024 * 1024,
        indexing_threads: 2,
        enable_semantic: true,
        languages: vec!["rust".to_string()],
        file_watch_debounce_ms: 500,
    };

    unsafe {
        std::env::set_var("RUNE_ENABLE_SEMANTIC", "true");
        std::env::set_var("QDRANT_URL", "http://127.0.0.1:6334");
    }

    let mut engine = RuneEngine::new(config).await.unwrap();
    engine.start().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Test pagination - first page
    let page1_query = rune_core::search::SearchQuery {
        query: "function process data".to_string(),
        mode: rune_core::search::SearchMode::Semantic,
        repositories: None,
        file_patterns: None,
        limit: 5,
        offset: 0,
    };

    let page1_results = engine.search().search(page1_query).await.unwrap();
    assert!(page1_results.results.len() <= 5, "Should respect limit");

    // Test pagination - second page
    let page2_query = rune_core::search::SearchQuery {
        query: "function process data".to_string(),
        mode: rune_core::search::SearchMode::Semantic,
        repositories: None,
        file_patterns: None,
        limit: 5,
        offset: 5,
    };

    let page2_results = engine.search().search(page2_query).await.unwrap();

    // Results should be different from page 1
    if !page2_results.results.is_empty() && !page1_results.results.is_empty() {
        assert_ne!(
            page1_results.results[0].file_path, page2_results.results[0].file_path,
            "Different pages should have different results"
        );
    }

    // Clean up
    unsafe {
        std::env::remove_var("RUNE_ENABLE_SEMANTIC");
        std::env::remove_var("QDRANT_URL");
    }
}
