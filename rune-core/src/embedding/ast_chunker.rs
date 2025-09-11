use anyhow::{Result, anyhow};
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;
use tracing::{debug, warn};
use tree_sitter::{Language as TSLanguage, Node, Parser, Query, QueryCursor};

use crate::indexing::language_detector::Language;

use super::chunker::{ChunkType, CodeChunk};

/// Configuration for AST-aware chunking
#[derive(Debug, Clone)]
pub struct AstChunkerConfig {
    /// Target chunk size in characters
    pub target_size: usize,
    /// Maximum chunk size (will split even semantic units if exceeded)
    pub max_size: usize,
    /// Minimum chunk size (won't create chunks smaller than this)
    pub min_size: usize,
    /// Whether to include imports/headers as context
    pub include_imports: bool,
    /// Whether to include parent context (e.g., class for methods)
    pub include_parent_context: bool,
    /// Overlap strategy for context preservation
    pub context_overlap: ContextOverlap,
}

#[derive(Debug, Clone)]
pub enum ContextOverlap {
    None,
    Minimal,    // Just function signatures
    Moderate,   // Include class/module headers
    Aggressive, // Include imports and parent context
}

impl Default for AstChunkerConfig {
    fn default() -> Self {
        Self {
            target_size: 1500, // ~512 tokens
            max_size: 3000,    // ~1024 tokens
            min_size: 200,     // ~64 tokens
            include_imports: true,
            include_parent_context: true,
            context_overlap: ContextOverlap::Moderate,
        }
    }
}

/// AST-aware code chunker using tree-sitter
pub struct AstChunker {
    config: AstChunkerConfig,
    parsers: HashMap<Language, Parser>,
    queries: HashMap<Language, ChunkingQueries>,
}

/// Language-specific queries for identifying chunk boundaries
struct ChunkingQueries {
    function_query: Query,
    class_query: Query,
    import_query: Query,
    _module_query: Option<Query>, // Kept for potential future module detection
}

impl AstChunker {
    pub fn new(config: AstChunkerConfig) -> Self {
        Self {
            config,
            parsers: HashMap::new(),
            queries: HashMap::new(),
        }
    }

    /// Chunk a file using AST analysis
    pub fn chunk_file(
        &mut self,
        content: &str,
        file_path: &str,
        language: Language,
    ) -> Result<Vec<CodeChunk>> {
        if !language.supports_tree_sitter() {
            return Err(anyhow!(
                "Language {:?} doesn't support tree-sitter parsing",
                language
            ));
        }

        // Parse the file
        let parser = self.get_or_create_parser(language)?;
        let tree = parser
            .parse(content, None)
            .ok_or_else(|| anyhow!("Failed to parse file"))?;

        let root = tree.root_node();

        // Extract semantic units based on language
        let semantic_units = self.extract_semantic_units(root, content, language)?;

        // Convert semantic units to chunks with appropriate context
        let chunks = self.units_to_chunks(semantic_units, content, file_path, language)?;

        debug!(
            "Created {} AST-based chunks for {}",
            chunks.len(),
            file_path
        );

        Ok(chunks)
    }

    /// Get or create a parser for the given language
    fn get_or_create_parser(&mut self, language: Language) -> Result<&mut Parser> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.parsers.entry(language) {
            let mut parser = Parser::new();
            let ts_language = get_tree_sitter_language(language)?;
            parser.set_language(&ts_language)?;
            e.insert(parser);

            // Initialize queries for this language
            self.initialize_queries(language, ts_language)?;
        }

        Ok(self.parsers.get_mut(&language).unwrap())
    }

    /// Initialize language-specific queries
    fn initialize_queries(&mut self, language: Language, ts_language: TSLanguage) -> Result<()> {
        let queries = match language {
            Language::Rust => ChunkingQueries {
                function_query: Query::new(
                    &ts_language,
                    r#"
                    (function_item
                        name: (identifier) @function.name
                        parameters: (parameters) @function.params
                        body: (block) @function.body) @function

                    (impl_item
                        type: (_) @impl.type
                        body: (declaration_list
                            (function_item
                                name: (identifier) @method.name
                                parameters: (parameters) @method.params
                                body: (block) @method.body) @method)) @impl
                    "#,
                )?,
                class_query: Query::new(
                    &ts_language,
                    r#"
                    (struct_item
                        name: (type_identifier) @struct.name
                        body: (_)? @struct.body) @struct

                    (enum_item
                        name: (type_identifier) @enum.name
                        body: (_) @enum.body) @enum

                    (trait_item
                        name: (type_identifier) @trait.name
                        body: (_) @trait.body) @trait
                    "#,
                )?,
                import_query: Query::new(&ts_language, r#"(use_declaration) @import"#)?,
                _module_query: Some(Query::new(&ts_language, r#"(mod_item) @module"#)?),
            },
            Language::Python => ChunkingQueries {
                function_query: Query::new(
                    &ts_language,
                    r#"
                    (function_definition
                        name: (identifier) @function.name
                        parameters: (parameters) @function.params
                        body: (_) @function.body) @function
                    "#,
                )?,
                class_query: Query::new(
                    &ts_language,
                    r#"
                    (class_definition
                        name: (identifier) @class.name
                        body: (_) @class.body) @class
                    "#,
                )?,
                import_query: Query::new(
                    &ts_language,
                    r#"
                    (import_statement) @import
                    (import_from_statement) @import
                    "#,
                )?,
                _module_query: None,
            },
            Language::JavaScript | Language::TypeScript => ChunkingQueries {
                function_query: Query::new(
                    &ts_language,
                    r#"
                    (function_declaration
                        name: (identifier) @function.name
                        parameters: (formal_parameters) @function.params
                        body: (statement_block) @function.body) @function

                    (arrow_function
                        parameters: (_) @arrow.params
                        body: (_) @arrow.body) @arrow

                    (method_definition
                        name: (_) @method.name
                        parameters: (formal_parameters) @method.params
                        body: (statement_block) @method.body) @method
                    "#,
                )?,
                class_query: Query::new(
                    &ts_language,
                    r#"
                    (class_declaration
                        name: (identifier) @class.name
                        body: (class_body) @class.body) @class
                    "#,
                )?,
                import_query: Query::new(
                    &ts_language,
                    r#"
                    (import_statement) @import
                    (export_statement) @export
                    "#,
                )?,
                _module_query: None,
            },
            Language::Go => ChunkingQueries {
                function_query: Query::new(
                    &ts_language,
                    r#"
                    (function_declaration
                        name: (identifier) @function.name
                        parameters: (parameter_list) @function.params
                        body: (block) @function.body) @function

                    (method_declaration
                        receiver: (parameter_list) @method.receiver
                        name: (field_identifier) @method.name
                        parameters: (parameter_list) @method.params
                        body: (block) @method.body) @method
                    "#,
                )?,
                class_query: Query::new(
                    &ts_language,
                    r#"
                    (type_declaration
                        (type_spec
                            name: (type_identifier) @type.name
                            type: (_) @type.body)) @type
                    "#,
                )?,
                import_query: Query::new(&ts_language, r#"(import_declaration) @import"#)?,
                _module_query: Some(Query::new(&ts_language, r#"(package_clause) @package"#)?),
            },
            _ => {
                // For other languages, create basic queries
                return Err(anyhow!(
                    "AST chunking not fully implemented for {:?}",
                    language
                ));
            },
        };

        self.queries.insert(language, queries);
        Ok(())
    }

    /// Extract semantic units from the AST
    fn extract_semantic_units(
        &mut self,
        root: Node,
        source: &str,
        language: Language,
    ) -> Result<Vec<SemanticUnit>> {
        let mut units = Vec::new();
        let queries = self
            .queries
            .get(&language)
            .ok_or_else(|| anyhow!("No queries initialized for {:?}", language))?;

        // Extract imports
        if self.config.include_imports {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&queries.import_query, root, source.as_bytes());
            while let Some(match_) = matches.next() {
                for capture in match_.captures {
                    units.push(SemanticUnit {
                        kind: SemanticUnitKind::Import,
                        _name: None,
                        start_byte: capture.node.start_byte(),
                        end_byte: capture.node.end_byte(),
                        start_line: capture.node.start_position().row + 1,
                        end_line: capture.node.end_position().row + 1,
                        _parent: None,
                    });
                }
            }
        }

        // Extract functions
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&queries.function_query, root, source.as_bytes());
            while let Some(match_) = matches.next() {
                if let Some(capture) = match_.captures.iter().find(|c| {
                    queries.function_query.capture_names()[c.index as usize] == "function"
                }) {
                    let name = extract_node_text(capture.node, source, "name");
                    units.push(SemanticUnit {
                        kind: SemanticUnitKind::Function,
                        _name: name,
                        start_byte: capture.node.start_byte(),
                        end_byte: capture.node.end_byte(),
                        start_line: capture.node.start_position().row + 1,
                        end_line: capture.node.end_position().row + 1,
                        _parent: None,
                    });
                }
            }
        }

        // Extract classes/structs
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&queries.class_query, root, source.as_bytes());
            while let Some(match_) = matches.next() {
                for capture in match_.captures {
                    let capture_name = &queries.class_query.capture_names()[capture.index as usize];
                    let kind = match capture_name {
                        s if s.starts_with("class") => SemanticUnitKind::Class,
                        s if s.starts_with("struct") => SemanticUnitKind::Struct,
                        s if s.starts_with("enum") => SemanticUnitKind::Enum,
                        s if s.starts_with("trait") => SemanticUnitKind::Trait,
                        _ => continue,
                    };

                    let name = extract_node_text(capture.node, source, "name");
                    units.push(SemanticUnit {
                        kind,
                        _name: name,
                        start_byte: capture.node.start_byte(),
                        end_byte: capture.node.end_byte(),
                        start_line: capture.node.start_position().row + 1,
                        end_line: capture.node.end_position().row + 1,
                        _parent: None,
                    });
                }
            }
        }

        // Sort units by position
        units.sort_by_key(|u| u.start_byte);

        Ok(units)
    }

    /// Convert semantic units to chunks with appropriate context
    fn units_to_chunks(
        &self,
        units: Vec<SemanticUnit>,
        source: &str,
        file_path: &str,
        language: Language,
    ) -> Result<Vec<CodeChunk>> {
        let mut chunks = Vec::new();
        let mut current_chunk = ChunkBuilder::new(file_path, language);

        // Collect imports as context
        let imports: Vec<_> = units
            .iter()
            .filter(|u| u.kind == SemanticUnitKind::Import)
            .collect();

        let import_context = if self.config.include_imports && !imports.is_empty() {
            imports
                .iter()
                .map(|u| &source[u.start_byte..u.end_byte])
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            String::new()
        };

        // Process non-import units
        for unit in units.iter().filter(|u| u.kind != SemanticUnitKind::Import) {
            let unit_content = &source[unit.start_byte..unit.end_byte];
            let unit_size = unit_content.len();

            // Check if this unit alone exceeds max size
            if unit_size > self.config.max_size {
                // Flush current chunk if not empty
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.build());
                    current_chunk = ChunkBuilder::new(file_path, language);
                }

                // Split large unit (this is a fallback for very large functions/classes)
                chunks.extend(self.split_large_unit(unit, source, file_path, language)?);
                continue;
            }

            // Check if adding this unit would exceed target size
            if !current_chunk.is_empty()
                && current_chunk.size() + unit_size > self.config.target_size
            {
                // Flush current chunk
                chunks.push(current_chunk.build());

                // Start new chunk with context
                current_chunk = ChunkBuilder::new(file_path, language);
                if !import_context.is_empty() {
                    current_chunk.add_context(&import_context);
                }
            }

            // Add unit to current chunk
            current_chunk.add_unit(unit, unit_content);
        }

        // Flush final chunk
        if !current_chunk.is_empty() {
            chunks.push(current_chunk.build());
        }

        // Ensure we have at least one chunk
        if chunks.is_empty() && !source.is_empty() {
            chunks.push(CodeChunk {
                content: source.to_string(),
                file_path: file_path.to_string(),
                start_line: 1,
                end_line: source.lines().count(),
                language: Some(language.to_str().to_string()),
                chunk_type: ChunkType::Block,
            });
        }

        Ok(chunks)
    }

    /// Split a large semantic unit into smaller chunks
    fn split_large_unit(
        &self,
        unit: &SemanticUnit,
        source: &str,
        file_path: &str,
        language: Language,
    ) -> Result<Vec<CodeChunk>> {
        warn!(
            "Splitting large {:?} unit ({}+ chars)",
            unit.kind, self.config.max_size
        );

        let unit_content = &source[unit.start_byte..unit.end_byte];
        let lines: Vec<&str> = unit_content.lines().collect();

        // Try to split at logical boundaries within the unit
        let mut chunks = Vec::new();
        let chunk_lines = self.config.target_size / 80; // Approximate chars per line

        for chunk_start in (0..lines.len()).step_by(chunk_lines) {
            let chunk_end = (chunk_start + chunk_lines).min(lines.len());
            let chunk_content = lines[chunk_start..chunk_end].join("\n");

            chunks.push(CodeChunk {
                content: chunk_content,
                file_path: file_path.to_string(),
                start_line: unit.start_line + chunk_start,
                end_line: unit.start_line + chunk_end - 1,
                language: Some(language.to_str().to_string()),
                chunk_type: unit.kind.to_chunk_type(),
            });
        }

        Ok(chunks)
    }
}

/// Helper to build chunks with context
struct ChunkBuilder {
    content: Vec<String>,
    file_path: String,
    language: Option<Language>,
    start_line: usize,
    end_line: usize,
    chunk_type: ChunkType,
}

impl ChunkBuilder {
    fn new(file_path: &str, language: Language) -> Self {
        Self {
            content: Vec::new(),
            file_path: file_path.to_string(),
            language: Some(language),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Block,
        }
    }

    fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    fn size(&self) -> usize {
        self.content.iter().map(|s| s.len()).sum()
    }

    fn add_context(&mut self, context: &str) {
        self.content.push(context.to_string());
    }

    fn add_unit(&mut self, unit: &SemanticUnit, content: &str) {
        if self.content.is_empty() {
            self.start_line = unit.start_line;
            self.chunk_type = unit.kind.to_chunk_type();
        }
        self.end_line = unit.end_line;
        self.content.push(content.to_string());
    }

    fn build(self) -> CodeChunk {
        CodeChunk {
            content: self.content.join("\n\n"),
            file_path: self.file_path,
            start_line: self.start_line,
            end_line: self.end_line,
            language: self.language.map(|l| l.to_str().to_string()),
            chunk_type: self.chunk_type,
        }
    }
}

/// Represents a semantic unit in the code
#[derive(Debug, Clone)]
struct SemanticUnit {
    kind: SemanticUnitKind,
    _name: Option<String>, // Kept for future semantic analysis
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    end_line: usize,
    _parent: Option<Box<SemanticUnit>>, // Kept for future hierarchical analysis
}

#[derive(Debug, Clone, PartialEq)]
enum SemanticUnitKind {
    Function,
    #[allow(dead_code)] // Will be used when method extraction is enhanced
    Method,
    Class,
    Struct,
    Enum,
    Trait,
    #[allow(dead_code)] // Will be used for TypeScript/Java interface support
    Interface,
    #[allow(dead_code)] // Will be used for module-level chunking
    Module,
    Import,
}

impl SemanticUnitKind {
    fn to_chunk_type(&self) -> ChunkType {
        match self {
            Self::Function | Self::Method => ChunkType::Function,
            Self::Class | Self::Struct | Self::Enum | Self::Trait | Self::Interface => {
                ChunkType::Class
            },
            Self::Module => ChunkType::Block,
            Self::Import => ChunkType::Import,
        }
    }
}

/// Helper to get tree-sitter language
fn get_tree_sitter_language(language: Language) -> Result<TSLanguage> {
    let lang = match language {
        Language::Rust => tree_sitter_rust::LANGUAGE,
        Language::JavaScript => tree_sitter_javascript::LANGUAGE,
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        Language::Python => tree_sitter_python::LANGUAGE,
        Language::Go => tree_sitter_go::LANGUAGE,
        Language::Java => tree_sitter_java::LANGUAGE,
        Language::Cpp | Language::C => tree_sitter_cpp::LANGUAGE,
        _ => {
            return Err(anyhow!(
                "Unsupported language for tree-sitter: {:?}",
                language
            ));
        },
    };

    Ok(lang.into())
}

/// Extract text for a specific field from a node
fn extract_node_text(node: Node, source: &str, field_name: &str) -> Option<String> {
    node.child_by_field_name(field_name)
        .map(|n| source[n.start_byte()..n.end_byte()].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_chunker_rust() {
        let code = r#"
use std::collections::HashMap;

pub struct MyStruct {
    field: String,
}

impl MyStruct {
    pub fn new() -> Self {
        Self {
            field: String::new(),
        }
    }

    pub fn process(&self, input: &str) -> String {
        format!("{}: {}", self.field, input)
    }
}

fn helper_function(x: i32) -> i32 {
    x * 2
}
"#;

        // Use smaller chunk size for testing to ensure multiple chunks
        let config = AstChunkerConfig {
            target_size: 150, // Small enough to split the test code
            max_size: 300,
            ..Default::default()
        };
        let mut chunker = AstChunker::new(config);
        let chunks = chunker.chunk_file(code, "test.rs", Language::Rust).unwrap();

        assert!(!chunks.is_empty());
        // Should have separate chunks for struct, impl block, and standalone function
        assert!(
            chunks.len() >= 2,
            "Expected at least 2 chunks, got {}",
            chunks.len()
        );
    }

    #[test]
    fn test_ast_chunker_python() {
        let code = r#"
import numpy as np
from typing import List

class DataProcessor:
    def __init__(self, name: str):
        self.name = name

    def process(self, data: List[float]) -> np.ndarray:
        return np.array(data) * 2

def standalone_function(x: int) -> int:
    """Helper function"""
    return x * 2
"#;

        // Use smaller chunk size for testing to ensure multiple chunks
        let config = AstChunkerConfig {
            target_size: 150, // Small enough to split the test code
            max_size: 300,
            ..Default::default()
        };
        let mut chunker = AstChunker::new(config);
        let chunks = chunker
            .chunk_file(code, "test.py", Language::Python)
            .unwrap();

        assert!(!chunks.is_empty());
        // Should separate class and standalone function
        assert!(
            chunks.len() >= 2,
            "Expected at least 2 chunks, got {}",
            chunks.len()
        );
    }

    #[test]
    fn test_large_function_splitting() {
        let mut large_function = String::from("fn very_large_function() {\n");
        for i in 0..200 {
            large_function.push_str(&format!("    let var_{} = {};\n", i, i));
        }
        large_function.push_str("}\n");

        let config = AstChunkerConfig {
            target_size: 500,
            max_size: 1000,
            ..Default::default()
        };

        let mut chunker = AstChunker::new(config);
        let chunks = chunker
            .chunk_file(&large_function, "test.rs", Language::Rust)
            .unwrap();

        // Should split the large function into multiple chunks
        assert!(chunks.len() > 1);
    }
}
