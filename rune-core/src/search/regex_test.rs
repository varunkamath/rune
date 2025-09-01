#[cfg(test)]
use crate::search::{SearchMode, SearchQuery, regex::RegexSearcher};
use crate::test_utils::{get_regex_test_files, setup_indexed_workspace};

async fn setup_test_searcher() -> (RegexSearcher, tempfile::TempDir) {
    // Use the regex-specific test files
    let files = get_regex_test_files();

    // Set up indexed workspace
    let (temp_dir, config, storage, tantivy_indexer) =
        setup_indexed_workspace(files).await.unwrap();

    // Create the regex searcher
    let searcher = RegexSearcher::new(config, storage, tantivy_indexer)
        .await
        .unwrap();

    (searcher, temp_dir)
}

#[tokio::test]
async fn test_simple_regex_pattern() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"data\d+".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(
        !results.is_empty(),
        "Should find matches for data with digits"
    );
    assert!(results.len() >= 3, "Should find data1, data2, data3");

    for result in &results {
        assert!(
            result.content.contains("data1")
                || result.content.contains("data2")
                || result.content.contains("data3"),
            "Each result should contain a data variable"
        );
    }
}

#[tokio::test]
async fn test_email_regex_pattern() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(!results.is_empty(), "Should find email addresses");

    let emails = ["user@example.com", "admin@test.org", "support@company.net"];
    for result in &results {
        let found_email = emails.iter().any(|email| result.content.contains(email));
        assert!(found_email, "Should match valid email patterns");
    }
}

#[tokio::test]
async fn test_phone_number_regex() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"\d{3}-\d{3}-\d{4}".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(!results.is_empty(), "Should find phone numbers");
    assert!(results.len() >= 3, "Should find all phone number patterns");
}

#[tokio::test]
async fn test_ip_address_regex() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(!results.is_empty(), "Should find IP addresses");
}

#[tokio::test]
async fn test_date_pattern_regex() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"\d{4}-\d{2}-\d{2}".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(!results.is_empty(), "Should find date patterns");
}

#[tokio::test]
async fn test_function_name_pattern() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"fn\s+\w+_error_\d+".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(!results.is_empty(), "Should find error handler functions");
    assert!(results.len() >= 3, "Should find handle_error_001, 002, 123");
}

#[tokio::test]
async fn test_variable_pattern() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"(user|admin)_\d+".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(!results.is_empty(), "Should find user and admin variables");
}

#[tokio::test]
async fn test_invalid_regex() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"[invalid(regex".to_string(), // Missing closing bracket
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let result = searcher.search(&query).await;

    assert!(result.is_err(), "Should return error for invalid regex");
}

#[tokio::test]
async fn test_case_insensitive_regex() {
    let (searcher, _temp) = setup_test_searcher().await;

    // Test if we can match both Error and error
    let query = SearchQuery {
        query: r"(?i)error".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(
        !results.is_empty(),
        "Should find matches case-insensitively"
    );
}

#[tokio::test]
async fn test_multiline_regex() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"fn\s+\w+\(\).*\{".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(!results.is_empty(), "Should find function definitions");
}

#[tokio::test]
async fn test_word_boundary_regex() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"\blet\b".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(
        !results.is_empty(),
        "Should find 'let' keyword with word boundaries"
    );
}

#[tokio::test]
async fn test_regex_with_file_filter() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"data\d+".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: Some(vec!["*.rs".to_string()]),
    };

    let results = searcher.search(&query).await.unwrap();

    for result in &results {
        assert!(
            result.file_path.extension().unwrap() == "rs",
            "Should only return results from Rust files"
        );
    }
}

#[tokio::test]
async fn test_complex_regex_pattern() {
    let (searcher, _temp) = setup_test_searcher().await;

    // Complex pattern: function names starting with handle_ followed by error_ and digits
    let query = SearchQuery {
        query: r"handle_error_\d{3}".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(!results.is_empty(), "Should find complex patterns");
    for result in &results {
        assert!(result.content.contains("handle_error_"));
    }
}

#[tokio::test]
async fn test_regex_caching() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: r"data\d+".to_string(),
        mode: SearchMode::Regex,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    // Run the same query multiple times
    let start = std::time::Instant::now();
    let _ = searcher.search(&query).await.unwrap();
    let first_duration = start.elapsed();

    let start = std::time::Instant::now();
    let _ = searcher.search(&query).await.unwrap();
    let second_duration = start.elapsed();

    // Second search should be faster due to caching
    // This is a weak assertion as timing can vary
    assert!(
        second_duration <= first_duration * 2,
        "Cached regex should not be significantly slower"
    );
}
