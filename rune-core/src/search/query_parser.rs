use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

use super::{SearchMode, SearchQuery};

/// Intent detected from a natural language query
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryIntent {
    /// Looking for function definitions
    FindFunction,
    /// Looking for class/struct definitions
    FindClass,
    /// Looking for variable/constant definitions
    FindVariable,
    /// Looking for imports/includes
    FindImport,
    /// Looking for error handling code
    FindErrorHandling,
    /// Looking for test code
    FindTests,
    /// Looking for documentation/comments
    FindDocumentation,
    /// General code search
    General,
}

/// Parsed components from a natural language query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedQuery {
    /// Original natural language query
    pub original: String,
    /// Detected intent
    pub intent: QueryIntent,
    /// Main search terms extracted
    pub keywords: Vec<String>,
    /// Language/file type filters detected
    pub language_filters: Vec<String>,
    /// Suggested search mode
    pub suggested_mode: SearchMode,
    /// Additional filters or modifiers
    pub modifiers: HashMap<String, String>,
}

/// Parser for natural language queries
pub struct QueryParser {
    /// Keywords that indicate function searches
    function_keywords: Vec<&'static str>,
    /// Keywords that indicate class/struct searches
    class_keywords: Vec<&'static str>,
    /// Keywords that indicate variable searches
    variable_keywords: Vec<&'static str>,
    /// Language mappings
    language_map: HashMap<&'static str, Vec<&'static str>>,
}

impl Default for QueryParser {
    fn default() -> Self {
        let mut language_map = HashMap::new();
        language_map.insert("rust", vec!["rs", "rust", "cargo"]);
        language_map.insert("python", vec!["py", "python", "pip", "django", "flask"]);
        language_map.insert(
            "javascript",
            vec!["js", "javascript", "node", "npm", "react", "vue"],
        );
        language_map.insert("typescript", vec!["ts", "typescript", "tsx"]);
        language_map.insert("go", vec!["go", "golang"]);
        language_map.insert("java", vec!["java", "maven", "gradle", "spring"]);
        language_map.insert("cpp", vec!["cpp", "c++", "cc", "cxx"]);

        Self {
            function_keywords: vec![
                "function",
                "func",
                "fn",
                "def",
                "method",
                "procedure",
                "proc",
                "handler",
                "callback",
                "hook",
                "middleware",
            ],
            class_keywords: vec![
                "class",
                "struct",
                "interface",
                "trait",
                "enum",
                "type",
                "model",
                "schema",
                "entity",
                "component",
            ],
            variable_keywords: vec![
                "variable",
                "var",
                "const",
                "constant",
                "let",
                "field",
                "property",
                "attribute",
                "member",
                "config",
                "setting",
            ],
            language_map,
        }
    }
}

impl QueryParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a natural language query into structured components
    pub fn parse(&self, query: &str) -> ParsedQuery {
        let query_lower = query.to_lowercase();
        let words: Vec<&str> = query_lower.split_whitespace().collect();

        // Detect intent
        let intent = self.detect_intent(&words);

        // Extract keywords (remove common words)
        let keywords = self.extract_keywords(&words, &intent);

        // Detect language filters
        let language_filters = self.detect_language_filters(&words);

        // Suggest search mode based on intent and keywords
        let suggested_mode = self.suggest_search_mode(&intent, &keywords, query);

        // Extract modifiers (like "all", "any", "exact")
        let modifiers = self.extract_modifiers(&words);

        debug!(
            "Parsed query '{}': intent={:?}, keywords={:?}, mode={:?}",
            query, intent, keywords, suggested_mode
        );

        ParsedQuery {
            original: query.to_string(),
            intent,
            keywords,
            language_filters,
            suggested_mode,
            modifiers,
        }
    }

    fn detect_intent(&self, words: &[&str]) -> QueryIntent {
        // Check for function keywords - also check for plural forms
        if words.iter().any(|w| {
            self.function_keywords.contains(w)
                || (w.len() > 1
                    && w.ends_with("s")
                    && self.function_keywords.contains(&&w[..w.len() - 1]))
        }) {
            return QueryIntent::FindFunction;
        }

        // Check for class keywords - also check for plural forms
        if words.iter().any(|w| {
            self.class_keywords.contains(w)
                || (w.len() > 2
                    && w.ends_with("es")
                    && self.class_keywords.contains(&&w[..w.len() - 2]))
                || (w.len() > 1
                    && w.ends_with("s")
                    && self.class_keywords.contains(&&w[..w.len() - 1]))
        }) {
            return QueryIntent::FindClass;
        }

        // Check for variable keywords
        if words.iter().any(|w| self.variable_keywords.contains(w)) {
            return QueryIntent::FindVariable;
        }

        // Check for specific patterns
        if words.iter().any(|w| {
            w.contains("import")
                || w.contains("include")
                || w.contains("use")
                || w.contains("require")
        }) {
            return QueryIntent::FindImport;
        }

        if words.iter().any(|w| {
            w.contains("error")
                || w.contains("exception")
                || w.contains("catch")
                || w.contains("try")
        }) {
            return QueryIntent::FindErrorHandling;
        }

        if words
            .iter()
            .any(|w| w.contains("test") || w.contains("spec") || w.contains("bench"))
        {
            return QueryIntent::FindTests;
        }

        if words.iter().any(|w| {
            w.contains("doc") || w.contains("comment") || w.contains("///") || w.contains("/**")
        }) {
            return QueryIntent::FindDocumentation;
        }

        QueryIntent::General
    }

    fn extract_keywords(&self, words: &[&str], intent: &QueryIntent) -> Vec<String> {
        let stop_words = vec![
            "find", "search", "look", "for", "all", "any", "the", "a", "an", "in", "on", "at",
            "to", "from", "with", "where", "that", "which", "show", "me", "get", "list", "display",
            "return",
        ];

        let mut keywords = Vec::new();

        for word in words {
            // Skip stop words
            if stop_words.contains(word) {
                continue;
            }

            // Skip intent-specific keywords we've already detected
            match intent {
                QueryIntent::FindFunction if self.function_keywords.contains(word) => continue,
                QueryIntent::FindClass if self.class_keywords.contains(word) => continue,
                QueryIntent::FindVariable if self.variable_keywords.contains(word) => continue,
                _ => {},
            }

            // Skip language keywords (we handle them separately) but not if it's the only keyword
            if words.len() > 1 && self.language_map.values().any(|langs| langs.contains(word)) {
                continue;
            }

            keywords.push(word.to_string());
        }

        // If we have no keywords after filtering, try to extract from camelCase or snake_case
        if keywords.is_empty() && !words.is_empty() {
            for word in words {
                if word.contains('_') || word.chars().any(|c| c.is_uppercase()) {
                    keywords.push(word.to_string());
                }
            }
        }

        keywords
    }

    fn detect_language_filters(&self, words: &[&str]) -> Vec<String> {
        let mut filters = Vec::new();

        for (lang, keywords) in &self.language_map {
            if words.iter().any(|w| keywords.contains(w)) {
                filters.push(lang.to_string());
            }
        }

        filters
    }

    fn suggest_search_mode(
        &self,
        intent: &QueryIntent,
        keywords: &[String],
        query: &str,
    ) -> SearchMode {
        // If query contains regex-like patterns, suggest regex mode
        if query.contains(".*")
            || query.contains("\\")
            || query.contains("[")
            || query.contains("^")
            || query.contains("$")
        {
            return SearchMode::Regex;
        }

        // For specific intents, prefer symbol search
        match intent {
            QueryIntent::FindFunction | QueryIntent::FindClass | QueryIntent::FindVariable => {
                SearchMode::Symbol
            },
            QueryIntent::FindImport | QueryIntent::FindErrorHandling => {
                // These might benefit from semantic search
                SearchMode::Semantic
            },
            _ => {
                // For general queries with multiple keywords, use hybrid
                if keywords.len() > 1 {
                    SearchMode::Hybrid
                } else {
                    SearchMode::Literal
                }
            },
        }
    }

    fn extract_modifiers(&self, words: &[&str]) -> HashMap<String, String> {
        let mut modifiers = HashMap::new();

        // Check for "all" vs "any"
        if words.contains(&"all") {
            modifiers.insert("match".to_string(), "all".to_string());
        } else if words.contains(&"any") {
            modifiers.insert("match".to_string(), "any".to_string());
        }

        // Check for case sensitivity
        if words.contains(&"exact") || words.contains(&"exactly") {
            modifiers.insert("case".to_string(), "sensitive".to_string());
        }

        // Check for scope modifiers
        if words.contains(&"public") {
            modifiers.insert("scope".to_string(), "public".to_string());
        } else if words.contains(&"private") {
            modifiers.insert("scope".to_string(), "private".to_string());
        }

        modifiers
    }

    /// Convert parsed query to search query
    pub fn to_search_query(&self, parsed: &ParsedQuery) -> SearchQuery {
        // Build the search string based on intent and keywords
        let query_string = match &parsed.intent {
            QueryIntent::FindFunction => {
                // For functions, search for function-like patterns
                if !parsed.keywords.is_empty() {
                    format!(
                        "fn {} | def {} | function {}",
                        parsed.keywords.join(" "),
                        parsed.keywords.join(" "),
                        parsed.keywords.join(" ")
                    )
                } else {
                    "fn | def | function".to_string()
                }
            },
            QueryIntent::FindClass => {
                if !parsed.keywords.is_empty() {
                    format!(
                        "class {} | struct {} | interface {}",
                        parsed.keywords.join(" "),
                        parsed.keywords.join(" "),
                        parsed.keywords.join(" ")
                    )
                } else {
                    "class | struct | interface".to_string()
                }
            },
            _ => {
                // For general queries, just use the keywords
                if !parsed.keywords.is_empty() {
                    parsed.keywords.join(" ")
                } else {
                    parsed.original.clone()
                }
            },
        };

        // Build file patterns from language filters
        let file_patterns = if !parsed.language_filters.is_empty() {
            Some(
                parsed
                    .language_filters
                    .iter()
                    .flat_map(|lang| match lang.as_str() {
                        "rust" => vec!["*.rs".to_string()],
                        "python" => vec!["*.py".to_string()],
                        "javascript" => vec!["*.js".to_string(), "*.jsx".to_string()],
                        "typescript" => vec!["*.ts".to_string(), "*.tsx".to_string()],
                        "go" => vec!["*.go".to_string()],
                        "java" => vec!["*.java".to_string()],
                        "cpp" => vec![
                            "*.cpp".to_string(),
                            "*.cc".to_string(),
                            "*.cxx".to_string(),
                            "*.h".to_string(),
                            "*.hpp".to_string(),
                        ],
                        _ => vec![],
                    })
                    .collect(),
            )
        } else {
            None
        };

        SearchQuery {
            query: query_string,
            mode: parsed.suggested_mode.clone(),
            repositories: None,
            file_patterns,
            limit: 50,
            offset: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function_query() {
        let parser = QueryParser::new();
        let parsed = parser.parse("find all functions that handle authentication");

        assert_eq!(parsed.intent, QueryIntent::FindFunction);
        assert!(parsed.keywords.contains(&"handle".to_string()));
        assert!(parsed.keywords.contains(&"authentication".to_string()));
        assert_eq!(parsed.suggested_mode, SearchMode::Symbol);
    }

    #[test]
    fn test_parse_class_query() {
        let parser = QueryParser::new();
        let parsed = parser.parse("show me all database model classes");

        assert_eq!(parsed.intent, QueryIntent::FindClass);
        assert!(parsed.keywords.contains(&"database".to_string()));
        assert_eq!(parsed.suggested_mode, SearchMode::Symbol);
    }

    #[test]
    fn test_language_detection() {
        let parser = QueryParser::new();
        let parsed = parser.parse("find all rust functions that use async");

        assert_eq!(parsed.language_filters, vec!["rust"]);
        assert!(parsed.keywords.contains(&"async".to_string()));
    }

    #[test]
    fn test_regex_detection() {
        let parser = QueryParser::new();
        let parsed = parser.parse("search for TODO.*fixme");

        assert_eq!(parsed.suggested_mode, SearchMode::Regex);
    }

    #[test]
    fn test_to_search_query() {
        let parser = QueryParser::new();
        let parsed = parser.parse("find all python functions named test");
        let search_query = parser.to_search_query(&parsed);

        assert!(search_query.query.contains("def"));
        assert!(search_query.query.contains("test") || search_query.query.contains("named"));
        assert_eq!(search_query.file_patterns, Some(vec!["*.py".to_string()]));
        assert_eq!(search_query.mode, SearchMode::Symbol);
    }
}
