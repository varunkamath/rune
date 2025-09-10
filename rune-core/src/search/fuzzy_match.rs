use strsim::{jaro_winkler, levenshtein, normalized_levenshtein};
use tracing::debug;

/// Configuration for fuzzy matching
#[derive(Debug, Clone)]
pub struct FuzzyConfig {
    /// Minimum similarity threshold (0.0-1.0) for a match to be considered
    pub similarity_threshold: f64,
    /// Maximum edit distance for Levenshtein matching
    pub max_edit_distance: usize,
    /// Whether to use Jaro-Winkler for better prefix matching
    pub use_jaro_winkler: bool,
    /// Whether fuzzy matching is enabled
    pub enabled: bool,
}

impl Default for FuzzyConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: std::env::var("RUNE_FUZZY_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.75), // Lowered from 0.8 for better typo tolerance
            max_edit_distance: std::env::var("RUNE_FUZZY_MAX_DISTANCE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2), // Lowered from 3 to be more strict
            use_jaro_winkler: std::env::var("RUNE_FUZZY_USE_JARO")
                .ok()
                .map(|s| s == "true" || s == "1")
                .unwrap_or(false),
            enabled: std::env::var("RUNE_FUZZY_ENABLED")
                .ok()
                .map(|s| s != "false" && s != "0")
                .unwrap_or(true),
        }
    }
}

/// Fuzzy string matcher for handling typos in searches
#[derive(Clone)]
pub struct FuzzyMatcher {
    config: FuzzyConfig,
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzyMatcher {
    /// Create a new fuzzy matcher with default configuration
    pub fn new() -> Self {
        Self {
            config: FuzzyConfig::default(),
        }
    }

    /// Create a fuzzy matcher with custom configuration
    pub fn with_config(config: FuzzyConfig) -> Self {
        Self { config }
    }

    /// Check if fuzzy matching is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Find fuzzy matches for a query in the given text
    pub fn find_fuzzy_matches(&self, query: &str, text: &str) -> Vec<FuzzyMatch> {
        if !self.config.enabled || query.is_empty() || text.is_empty() {
            return vec![];
        }

        let mut matches = Vec::new();
        let query_lower = query.to_lowercase();
        let text_lower = text.to_lowercase();

        // Split text into words for word-level fuzzy matching
        // Use a more comprehensive split that handles punctuation
        let words: Vec<&str> = text_lower
            .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .filter(|s| !s.is_empty())
            .collect();

        for word in &words {
            // Skip very short words unless the query is also short
            if word.len() < 2 && query_lower.len() > 2 {
                continue;
            }

            // Calculate similarity
            let similarity = if self.config.use_jaro_winkler {
                jaro_winkler(&query_lower, word)
            } else {
                normalized_levenshtein(&query_lower, word)
            };

            // Calculate edit distance
            let edit_distance = levenshtein(&query_lower, word);

            // Check if this is a good match
            if similarity >= self.config.similarity_threshold
                && edit_distance <= self.config.max_edit_distance
            {
                // Find the position of this word in the original text
                let position = text_lower.find(word).unwrap_or(0);

                debug!(
                    "Fuzzy match found: '{}' ~ '{}' (similarity: {:.2}, distance: {})",
                    query, word, similarity, edit_distance
                );

                matches.push(FuzzyMatch {
                    matched_text: word.to_string(),
                    similarity,
                    edit_distance,
                    position,
                });
            }
        }

        // Also check for substring fuzzy matching for compound words
        // Only if we haven't found good word matches
        if matches.is_empty() {
            matches.extend(self.find_substring_fuzzy_matches(&query_lower, &text_lower));
        }

        // Sort by similarity (highest first)
        matches.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Remove duplicates based on position
        matches.dedup_by(|a, b| a.position == b.position);

        matches
    }

    /// Find fuzzy matches within substrings (for compound words like "getElementById")
    fn find_substring_fuzzy_matches(&self, query: &str, text: &str) -> Vec<FuzzyMatch> {
        let mut matches = Vec::new();

        // Only do substring matching for reasonably sized queries
        if query.len() < 3 || query.len() > 30 {
            return matches;
        }

        // Convert to char indices to handle UTF-8 properly
        let text_chars: Vec<char> = text.chars().collect();
        let query_char_len = query.chars().count();

        if text_chars.is_empty() || query_char_len == 0 {
            return matches;
        }

        // Slide a window through the text looking for approximate matches
        // Window size should be close to query length in characters
        let min_window = query_char_len.saturating_sub(self.config.max_edit_distance);
        let max_window = query_char_len + self.config.max_edit_distance;

        for window_size in min_window..=max_window {
            for i in 0..text_chars.len().saturating_sub(window_size - 1) {
                let end = (i + window_size).min(text_chars.len());
                let substring: String = text_chars[i..end].iter().collect();

                // Calculate similarity for this substring
                let similarity = if self.config.use_jaro_winkler {
                    jaro_winkler(query, &substring)
                } else {
                    normalized_levenshtein(query, &substring)
                };

                let edit_distance = levenshtein(query, &substring);

                if similarity >= self.config.similarity_threshold
                    && edit_distance <= self.config.max_edit_distance
                {
                    // Calculate byte position from char position
                    let byte_position = text.char_indices().nth(i).map(|(pos, _)| pos).unwrap_or(0);

                    matches.push(FuzzyMatch {
                        matched_text: substring,
                        similarity,
                        edit_distance,
                        position: byte_position,
                    });
                }
            }
        }

        matches
    }

    /// Find the byte position of a word in the original text
    fn find_word_position(&self, text: &str, word_index: usize) -> usize {
        let mut current_word = 0;
        let mut position = 0;

        for (i, ch) in text.char_indices() {
            if ch.is_whitespace() {
                if position > 0
                    && !text
                        .chars()
                        .nth(position - 1)
                        .unwrap_or(' ')
                        .is_whitespace()
                {
                    current_word += 1;
                }
            } else if current_word == word_index
                && (i == 0 || text.chars().nth(i - 1).unwrap_or(' ').is_whitespace())
            {
                return i;
            }
            position = i;
        }

        0
    }

    /// Check if a single word is a fuzzy match for the query
    pub fn is_fuzzy_match(&self, query: &str, word: &str) -> bool {
        if !self.config.enabled {
            return false;
        }

        let similarity = if self.config.use_jaro_winkler {
            jaro_winkler(&query.to_lowercase(), &word.to_lowercase())
        } else {
            normalized_levenshtein(&query.to_lowercase(), &word.to_lowercase())
        };

        let edit_distance = levenshtein(&query.to_lowercase(), &word.to_lowercase());

        similarity >= self.config.similarity_threshold
            && edit_distance <= self.config.max_edit_distance
    }

    /// Get similarity score between two strings
    pub fn similarity(&self, s1: &str, s2: &str) -> f64 {
        if self.config.use_jaro_winkler {
            jaro_winkler(&s1.to_lowercase(), &s2.to_lowercase())
        } else {
            normalized_levenshtein(&s1.to_lowercase(), &s2.to_lowercase())
        }
    }
}

/// Represents a fuzzy match found in text
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    /// The text that was matched
    pub matched_text: String,
    /// Similarity score (0.0-1.0)
    pub similarity: f64,
    /// Edit distance between query and match
    pub edit_distance: usize,
    /// Position in the original text
    pub position: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_fuzzy_match() {
        let matcher = FuzzyMatcher::new();

        // Test basic typo
        assert!(matcher.is_fuzzy_match("function", "functoin"));
        assert!(matcher.is_fuzzy_match("variable", "varaible"));
        assert!(matcher.is_fuzzy_match("implement", "implment"));
    }

    #[test]
    fn test_similarity_threshold() {
        // Test with high threshold - requires very similar strings
        let high_config = FuzzyConfig {
            similarity_threshold: 0.95,
            max_edit_distance: 5, // Allow enough edits to test threshold properly
            ..Default::default()
        };
        let high_matcher = FuzzyMatcher::with_config(high_config);

        // Should not match with high threshold and very different strings
        assert!(!high_matcher.is_fuzzy_match("function", "fXXXXXXX"));

        // Test with medium threshold for reasonable matching
        let medium_config = FuzzyConfig {
            similarity_threshold: 0.85,
            max_edit_distance: 3,
            ..Default::default()
        };
        let medium_matcher = FuzzyMatcher::with_config(medium_config);

        // Should match with very similar strings (1 char addition)
        assert!(medium_matcher.is_fuzzy_match("function", "functions"));
    }

    #[test]
    fn test_find_fuzzy_matches_in_text() {
        let matcher = FuzzyMatcher::new();
        let text = "This functoin processes the varaible and returns a value";

        let matches = matcher.find_fuzzy_matches("function", text);
        assert!(!matches.is_empty());
        assert_eq!(matches[0].matched_text, "functoin");

        let matches = matcher.find_fuzzy_matches("variable", text);
        assert!(!matches.is_empty());
        assert_eq!(matches[0].matched_text, "varaible");
    }

    #[test]
    fn test_edit_distance_limit() {
        let config = FuzzyConfig {
            max_edit_distance: 2,      // Allow 2 edits
            similarity_threshold: 0.5, // Lower threshold for this test
            ..Default::default()
        };
        let matcher = FuzzyMatcher::with_config(config);

        // 2 edit distance - should match with relaxed settings
        assert!(matcher.is_fuzzy_match("test", "tets"));

        // Now test with strict 1 edit limit
        let strict_config = FuzzyConfig {
            max_edit_distance: 1,
            similarity_threshold: 0.5,
            ..Default::default()
        };
        let strict_matcher = FuzzyMatcher::with_config(strict_config);

        // 1 edit distance - should match
        assert!(strict_matcher.is_fuzzy_match("test", "tesr")); // 1 substitution
        // 3 edit distance - should not match
        assert!(!strict_matcher.is_fuzzy_match("test", "xxxx"));
    }

    #[test]
    fn test_compound_word_matching() {
        let matcher = FuzzyMatcher::new();
        let text = "getElementById is a common DOM method";

        let matches = matcher.find_fuzzy_matches("getElementByID", text);
        assert!(!matches.is_empty());
    }

    #[test]
    fn test_case_insensitive_matching() {
        let matcher = FuzzyMatcher::new();

        assert!(matcher.is_fuzzy_match("Function", "functoin"));
        assert!(matcher.is_fuzzy_match("VARIABLE", "varaible"));
    }

    #[test]
    fn test_jaro_winkler_mode() {
        let config = FuzzyConfig {
            use_jaro_winkler: true,
            ..Default::default()
        };
        let matcher = FuzzyMatcher::with_config(config);

        // Jaro-Winkler gives higher scores to strings with common prefixes
        let score1 = matcher.similarity("function", "functoin");
        let score2 = matcher.similarity("function", "nfunction");
        assert!(score1 > score2);
    }

    #[test]
    fn test_disabled_fuzzy_matching() {
        let config = FuzzyConfig {
            enabled: false,
            ..Default::default()
        };
        let matcher = FuzzyMatcher::with_config(config);

        assert!(!matcher.is_fuzzy_match("function", "functoin"));
        assert!(
            matcher
                .find_fuzzy_matches("function", "functoin test")
                .is_empty()
        );
    }
}
