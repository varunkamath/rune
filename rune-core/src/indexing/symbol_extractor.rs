use std::path::Path;

use anyhow::{Result, anyhow};
use tree_sitter::{Language as TSLanguage, Node, Parser};

use super::language_detector::Language;

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Interface,
    Struct,
    Enum,
    Module,
    Namespace,
    Variable,
    Constant,
    Field,
    Property,
    Type,
    Trait,
    Implementation,
}

impl SymbolKind {
    pub fn to_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Module => "module",
            SymbolKind::Namespace => "namespace",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Field => "field",
            SymbolKind::Property => "property",
            SymbolKind::Type => "type",
            SymbolKind::Trait => "trait",
            SymbolKind::Implementation => "impl",
        }
    }
}

pub struct SymbolExtractor {
    parsers: dashmap::DashMap<Language, Parser>,
}

impl Default for SymbolExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolExtractor {
    pub fn new() -> Self {
        Self {
            parsers: dashmap::DashMap::new(),
        }
    }

    pub fn extract_symbols(
        &self,
        _path: &Path,
        content: &str,
        language: Language,
    ) -> Result<Vec<Symbol>> {
        if !language.supports_tree_sitter() {
            return Ok(Vec::new());
        }

        // Get or create parser for this language, then parse with exclusive access
        self.ensure_parser_exists(language)?;

        // Use DashMap's entry API to get exclusive mutable access to cached parser
        let tree = {
            let mut parser_ref = self
                .parsers
                .get_mut(&language)
                .ok_or_else(|| anyhow!("Parser not found after creation"))?;
            parser_ref
                .parse(content, None)
                .ok_or_else(|| anyhow!("Failed to parse file"))?
        }; // parser_ref dropped here, releasing the lock

        let root = tree.root_node();
        let mut symbols = Vec::new();

        match language {
            Language::Rust => self.extract_rust_symbols(root, content, &mut symbols)?,
            Language::JavaScript | Language::TypeScript => {
                self.extract_javascript_symbols(root, content, &mut symbols)?
            },
            Language::Python => self.extract_python_symbols(root, content, &mut symbols)?,
            Language::Go => self.extract_go_symbols(root, content, &mut symbols)?,
            Language::Java => self.extract_java_symbols(root, content, &mut symbols)?,
            Language::Cpp | Language::C => self.extract_c_symbols(root, content, &mut symbols)?,
            _ => {
                // Generic extraction for other languages
                self.extract_generic_symbols(root, content, &mut symbols)?;
            },
        }

        Ok(symbols)
    }

    fn ensure_parser_exists(&self, language: Language) -> Result<()> {
        // Check if parser already exists in cache
        if self.parsers.contains_key(&language) {
            return Ok(());
        }

        // Create and cache a new parser for this language
        let mut parser = Parser::new();
        let ts_language = self.get_tree_sitter_language(language)?;
        parser.set_language(&ts_language)?;
        self.parsers.insert(language, parser);

        Ok(())
    }

    fn get_tree_sitter_language(&self, language: Language) -> Result<TSLanguage> {
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

    fn extract_rust_symbols(
        &self,
        node: Node,
        source: &str,
        symbols: &mut Vec<Symbol>,
    ) -> Result<()> {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_item" | "method_item" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Function,
                            child,
                            source,
                        )?);
                    }
                },
                "struct_item" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Struct,
                            child,
                            source,
                        )?);
                    }
                },
                "enum_item" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(name, SymbolKind::Enum, child, source)?);
                    }
                },
                "trait_item" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(name, SymbolKind::Trait, child, source)?);
                    }
                },
                "impl_item" => {
                    let type_node = child.child_by_field_name("type");
                    if let Some(type_node) = type_node {
                        let name = type_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Implementation,
                            child,
                            source,
                        )?);
                    }
                },
                "mod_item" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Module,
                            child,
                            source,
                        )?);
                    }
                },
                "const_item" | "static_item" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Constant,
                            child,
                            source,
                        )?);
                    }
                },
                _ => {
                    // Recursively extract from child nodes
                    self.extract_rust_symbols(child, source, symbols)?;
                },
            }
        }

        Ok(())
    }

    fn extract_javascript_symbols(
        &self,
        node: Node,
        source: &str,
        symbols: &mut Vec<Symbol>,
    ) -> Result<()> {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_declaration" | "function_expression" | "arrow_function" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Function,
                            child,
                            source,
                        )?);
                    }
                },
                "class_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(name, SymbolKind::Class, child, source)?);
                    }
                },
                "method_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Method,
                            child,
                            source,
                        )?);
                    }
                },
                "variable_declarator" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Variable,
                            child,
                            source,
                        )?);
                    }
                },
                _ => {
                    self.extract_javascript_symbols(child, source, symbols)?;
                },
            }
        }

        Ok(())
    }

    fn extract_python_symbols(
        &self,
        node: Node,
        source: &str,
        symbols: &mut Vec<Symbol>,
    ) -> Result<()> {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Function,
                            child,
                            source,
                        )?);
                    }
                },
                "class_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(name, SymbolKind::Class, child, source)?);
                    }
                },
                _ => {
                    self.extract_python_symbols(child, source, symbols)?;
                },
            }
        }

        Ok(())
    }

    fn extract_go_symbols(
        &self,
        node: Node,
        source: &str,
        symbols: &mut Vec<Symbol>,
    ) -> Result<()> {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_declaration" | "method_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Function,
                            child,
                            source,
                        )?);
                    }
                },
                "type_declaration" => {
                    if let Some(spec) = child.child_by_field_name("type_spec")
                        && let Some(name_node) = spec.child_by_field_name("name")
                    {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        let kind = if spec
                            .child_by_field_name("type")
                            .map(|t| t.kind() == "struct_type")
                            .unwrap_or(false)
                        {
                            SymbolKind::Struct
                        } else if spec
                            .child_by_field_name("type")
                            .map(|t| t.kind() == "interface_type")
                            .unwrap_or(false)
                        {
                            SymbolKind::Interface
                        } else {
                            SymbolKind::Type
                        };
                        symbols.push(self.create_symbol(name, kind, child, source)?);
                    }
                },
                _ => {
                    self.extract_go_symbols(child, source, symbols)?;
                },
            }
        }

        Ok(())
    }

    fn extract_java_symbols(
        &self,
        node: Node,
        source: &str,
        symbols: &mut Vec<Symbol>,
    ) -> Result<()> {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "method_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Method,
                            child,
                            source,
                        )?);
                    }
                },
                "class_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(name, SymbolKind::Class, child, source)?);
                    }
                },
                "interface_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Interface,
                            child,
                            source,
                        )?);
                    }
                },
                "enum_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(name, SymbolKind::Enum, child, source)?);
                    }
                },
                _ => {
                    self.extract_java_symbols(child, source, symbols)?;
                },
            }
        }

        Ok(())
    }

    fn extract_c_symbols(&self, node: Node, source: &str, symbols: &mut Vec<Symbol>) -> Result<()> {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some(declarator) = child.child_by_field_name("declarator")
                        && let Some(name_node) = Self::find_identifier(declarator)
                    {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Function,
                            child,
                            source,
                        )?);
                    }
                },
                "struct_specifier" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(
                            name,
                            SymbolKind::Struct,
                            child,
                            source,
                        )?);
                    }
                },
                "enum_specifier" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = name_node.utf8_text(source.as_bytes())?;
                        symbols.push(self.create_symbol(name, SymbolKind::Enum, child, source)?);
                    }
                },
                _ => {
                    self.extract_c_symbols(child, source, symbols)?;
                },
            }
        }

        Ok(())
    }

    fn extract_generic_symbols(
        &self,
        node: Node,
        source: &str,
        symbols: &mut Vec<Symbol>,
    ) -> Result<()> {
        // Generic extraction based on common patterns
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            let kind_str = child.kind();

            // Try to identify symbols based on node kind
            if kind_str.contains("function") || kind_str.contains("method") {
                if let Some(name) = self.extract_node_name(child, source) {
                    symbols.push(self.create_symbol(&name, SymbolKind::Function, child, source)?);
                }
            } else if kind_str.contains("class") {
                if let Some(name) = self.extract_node_name(child, source) {
                    symbols.push(self.create_symbol(&name, SymbolKind::Class, child, source)?);
                }
            } else if kind_str.contains("struct")
                && let Some(name) = self.extract_node_name(child, source)
            {
                symbols.push(self.create_symbol(&name, SymbolKind::Struct, child, source)?);
            }

            // Recurse
            self.extract_generic_symbols(child, source, symbols)?;
        }

        Ok(())
    }

    fn find_identifier(node: Node) -> Option<Node> {
        if node.kind() == "identifier" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(id) = Self::find_identifier(child) {
                return Some(id);
            }
        }

        None
    }

    fn extract_node_name(&self, node: Node, source: &str) -> Option<String> {
        // Try common field names
        for field_name in &["name", "identifier", "id"] {
            if let Some(name_node) = node.child_by_field_name(field_name)
                && let Ok(name) = name_node.utf8_text(source.as_bytes())
            {
                return Some(name.to_string());
            }
        }

        // Try to find an identifier child
        if let Some(id_node) = Self::find_identifier(node)
            && let Ok(name) = id_node.utf8_text(source.as_bytes())
        {
            return Some(name.to_string());
        }

        None
    }

    fn create_symbol(
        &self,
        name: &str,
        kind: SymbolKind,
        node: Node,
        source: &str,
    ) -> Result<Symbol> {
        let start_pos = node.start_position();
        let end_pos = node.end_position();

        // Extract signature (first line of the symbol definition)
        let signature = {
            let start_byte = node.start_byte();
            let end_byte = node.end_byte().min(start_byte + 200); // Limit signature length
            let text = &source.as_bytes()[start_byte..end_byte];

            if let Ok(sig) = std::str::from_utf8(text) {
                sig.lines().next().map(|s| s.to_string())
            } else {
                None
            }
        };

        Ok(Symbol {
            name: name.to_string(),
            kind,
            start_line: start_pos.row,
            end_line: end_pos.row,
            start_col: start_pos.column,
            end_col: end_pos.column,
            signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_symbols() {
        let source = r#"
            fn main() {
                println!("Hello");
            }

            struct MyStruct {
                field: String,
            }

            impl MyStruct {
                fn new() -> Self {
                    Self { field: String::new() }
                }
            }
        "#;

        let extractor = SymbolExtractor::new();
        let symbols = extractor
            .extract_symbols(Path::new("test.rs"), source, Language::Rust)
            .unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "main" && s.kind == SymbolKind::Function)
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MyStruct" && s.kind == SymbolKind::Struct)
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MyStruct" && s.kind == SymbolKind::Implementation)
        );
    }
}
