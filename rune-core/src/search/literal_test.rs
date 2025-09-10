#[cfg(test)]
use crate::search::{SearchMode, SearchQuery, literal::LiteralSearcher};
use crate::test_utils::{get_standard_test_files, setup_indexed_workspace};
use std::sync::Arc;

async fn setup_test_searcher() -> (LiteralSearcher, tempfile::TempDir) {
    // Use the standard test files
    let files = get_standard_test_files();

    // Set up indexed workspace
    let (temp_dir, config, storage, tantivy_indexer) =
        setup_indexed_workspace(files).await.unwrap();

    // Create the literal searcher
    let searcher = LiteralSearcher::new(config, storage, tantivy_indexer)
        .await
        .unwrap();

    (searcher, temp_dir)
}

#[tokio::test]
async fn test_basic_literal_search() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: "calculate_sum".to_string(),
        mode: SearchMode::Literal,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(
        !results.is_empty(),
        "Should find results for 'calculate_sum'"
    );
    assert!(results.len() >= 2, "Should find at least 2 occurrences");

    // Check that we found it in multiple files
    let file_paths: Vec<String> = results
        .iter()
        .map(|r| {
            r.file_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        })
        .collect();

    // We should find it in at least Rust and Python files
    assert!(file_paths.iter().any(|f| f.ends_with(".rs")));
    assert!(file_paths.iter().any(|f| f.ends_with(".py")));
}

#[tokio::test]
async fn test_case_insensitive_search() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: "CALCULATE_SUM".to_string(),
        mode: SearchMode::Literal,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    // Tantivy should handle case-insensitive search by default
    assert!(
        !results.is_empty(),
        "Should find results with different case"
    );
}

#[tokio::test]
async fn test_partial_word_search() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: "calculate".to_string(),
        mode: SearchMode::Literal,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(
        results.len() >= 3,
        "Should find all functions with 'calculate' prefix"
    );
}

#[tokio::test]
async fn test_search_with_limit() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: "calculate".to_string(),
        mode: SearchMode::Literal,
        limit: 2,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert_eq!(results.len(), 2, "Should respect the limit parameter");
}

#[tokio::test]
async fn test_search_with_offset() {
    let (searcher, _temp) = setup_test_searcher().await;

    // First get all results
    let query_all = SearchQuery {
        query: "calculate".to_string(),
        mode: SearchMode::Literal,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };
    let all_results = searcher.search(&query_all).await.unwrap();

    // Then get results with offset
    let query_offset = SearchQuery {
        query: "calculate".to_string(),
        mode: SearchMode::Literal,
        limit: 10,
        offset: 1,
        repositories: None,
        file_patterns: None,
    };
    let offset_results = searcher.search(&query_offset).await.unwrap();

    // Verify offset works correctly
    // With offset=1, we should skip the first result but still get up to limit results
    if all_results.len() > 1 {
        // Check that the first result of offset query matches the second result of the original query
        assert_eq!(
            offset_results[0].file_path, all_results[1].file_path,
            "First result with offset=1 should match second result without offset"
        );
        assert_eq!(
            offset_results[0].line_number, all_results[1].line_number,
            "Line numbers should match when offset is applied"
        );
    }
}

#[tokio::test]
async fn test_search_with_file_patterns() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: "calculate".to_string(),
        mode: SearchMode::Literal,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: Some(vec!["*.rs".to_string()]),
    };

    let results = searcher.search(&query).await.unwrap();

    // Should only find results in Rust files
    for result in &results {
        assert!(
            result.file_path.extension().unwrap() == "rs",
            "Should only return .rs files"
        );
    }
}

#[tokio::test]
async fn test_empty_query() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: "".to_string(),
        mode: SearchMode::Literal,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await;

    // Empty query should either return an error or empty results
    assert!(results.is_err() || results.unwrap().is_empty());
}

#[tokio::test]
async fn test_special_characters_in_query() {
    let (searcher, _temp) = setup_test_searcher().await;

    // Test with special characters that might need escaping
    let queries = vec!["a + b", "->", "/**", "\"\"\""];

    for query_str in queries {
        let query = SearchQuery {
            query: query_str.to_string(),
            mode: SearchMode::Literal,
            limit: 10,
            offset: 0,
            repositories: None,
            file_patterns: None,
        };

        // Should handle special characters gracefully
        let _ = searcher.search(&query).await;
    }
}

#[tokio::test]
async fn test_context_extraction() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: "calculate_sum".to_string(),
        mode: SearchMode::Literal,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    for result in results {
        // Should have context lines
        assert!(
            !result.context_before.is_empty() || !result.context_after.is_empty(),
            "Should include context lines"
        );

        // For exact matches, content should contain the search term
        // For fuzzy matches, content may contain a similar term
        if result.match_type == crate::search::MatchType::Exact {
            assert!(
                result.content.contains("calculate_sum"),
                "Exact match content should contain the search term"
            );
        }
        // For fuzzy matches, we just verify that the result has content
        assert!(!result.content.is_empty(), "Result should have content");
    }
}

#[tokio::test]
async fn test_no_results_query() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: "nonexistent_function_xyz".to_string(),
        mode: SearchMode::Literal,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    assert!(
        results.is_empty(),
        "Should return empty results for non-existent term"
    );
}

#[tokio::test]
async fn test_search_scoring() {
    let (searcher, _temp) = setup_test_searcher().await;

    let query = SearchQuery {
        query: "calculate_sum".to_string(),
        mode: SearchMode::Literal,
        limit: 10,
        offset: 0,
        repositories: None,
        file_patterns: None,
    };

    let results = searcher.search(&query).await.unwrap();

    // All results should have a score
    for result in results {
        assert!(
            result.score > 0.0,
            "Each result should have a positive score"
        );
    }
}

#[tokio::test]
async fn test_concurrent_searches() {
    let (searcher, _temp) = setup_test_searcher().await;
    let searcher = Arc::new(searcher);

    let mut handles = vec![];

    for _i in 0..5 {
        let searcher_clone = searcher.clone();
        let handle = tokio::spawn(async move {
            let query = SearchQuery {
                query: "calculate".to_string(),
                mode: SearchMode::Literal,
                limit: 10,
                offset: 0,
                repositories: None,
                file_patterns: None,
            };
            searcher_clone.search(&query).await
        });
        handles.push(handle);
    }

    // All concurrent searches should succeed
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent search should succeed");
    }
}
