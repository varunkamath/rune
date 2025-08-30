use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    Java,
    Cpp,
    C,
    CSharp,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Scala,
    Haskell,
    Elixir,
    Html,
    Css,
    Markdown,
    Json,
    Yaml,
    Toml,
    Xml,
    Shell,
    Unknown,
}

impl Language {
    pub fn from_path(path: &Path) -> Self {
        // First check file extension
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            return Self::from_extension(&ext);
        }

        // Check filename for extensionless files
        if let Some(name) = path.file_name() {
            let name = name.to_string_lossy();
            return Self::from_filename(&name);
        }

        Language::Unknown
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "rs" => Language::Rust,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "ts" | "tsx" => Language::TypeScript,
            "jsx" => Language::JavaScript,
            "py" | "pyw" => Language::Python,
            "go" => Language::Go,
            "java" => Language::Java,
            "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hxx" | "h++" => Language::Cpp,
            "c" | "h" => Language::C,
            "cs" => Language::CSharp,
            "rb" => Language::Ruby,
            "php" => Language::Php,
            "swift" => Language::Swift,
            "kt" | "kts" => Language::Kotlin,
            "scala" | "sc" => Language::Scala,
            "hs" | "lhs" => Language::Haskell,
            "ex" | "exs" => Language::Elixir,
            "html" | "htm" => Language::Html,
            "css" | "scss" | "sass" | "less" => Language::Css,
            "md" | "markdown" => Language::Markdown,
            "json" => Language::Json,
            "yml" | "yaml" => Language::Yaml,
            "toml" => Language::Toml,
            "xml" => Language::Xml,
            "sh" | "bash" | "zsh" | "fish" => Language::Shell,
            _ => Language::Unknown,
        }
    }

    pub fn from_filename(name: &str) -> Self {
        match name {
            "Dockerfile" => Language::Shell,
            "Makefile" | "makefile" => Language::Shell,
            "Gemfile" | "Rakefile" => Language::Ruby,
            "package.json" | "tsconfig.json" => Language::Json,
            "Cargo.toml" => Language::Toml,
            "go.mod" | "go.sum" => Language::Go,
            "pom.xml" => Language::Xml,
            "build.gradle" | "settings.gradle" => Language::Java,
            _ => Language::Unknown,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Python => "python",
            Language::Go => "go",
            Language::Java => "java",
            Language::Cpp => "cpp",
            Language::C => "c",
            Language::CSharp => "csharp",
            Language::Ruby => "ruby",
            Language::Php => "php",
            Language::Swift => "swift",
            Language::Kotlin => "kotlin",
            Language::Scala => "scala",
            Language::Haskell => "haskell",
            Language::Elixir => "elixir",
            Language::Html => "html",
            Language::Css => "css",
            Language::Markdown => "markdown",
            Language::Json => "json",
            Language::Yaml => "yaml",
            Language::Toml => "toml",
            Language::Xml => "xml",
            Language::Shell => "shell",
            Language::Unknown => "unknown",
        }
    }

    pub fn get_comment_syntax(&self) -> CommentSyntax {
        match self {
            Language::Rust
            | Language::Go
            | Language::Java
            | Language::Cpp
            | Language::C
            | Language::CSharp
            | Language::Swift
            | Language::Kotlin
            | Language::Scala
            | Language::JavaScript
            | Language::TypeScript => CommentSyntax::CStyle,

            Language::Python
            | Language::Ruby
            | Language::Shell
            | Language::Yaml
            | Language::Toml
            | Language::Elixir => CommentSyntax::Hash,

            Language::Haskell => CommentSyntax::DoubleDash,
            Language::Html | Language::Xml => CommentSyntax::Xml,
            Language::Css => CommentSyntax::CssStyle,
            Language::Php => CommentSyntax::Mixed,
            Language::Markdown | Language::Json | Language::Unknown => CommentSyntax::None,
        }
    }

    pub fn supports_tree_sitter(&self) -> bool {
        matches!(
            self,
            Language::Rust
                | Language::JavaScript
                | Language::TypeScript
                | Language::Python
                | Language::Go
                | Language::Java
                | Language::Cpp
                | Language::C
                | Language::CSharp
                | Language::Ruby
                | Language::Php
                | Language::Html
                | Language::Css
                | Language::Json
                | Language::Yaml
                | Language::Toml
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CommentSyntax {
    CStyle,     // // and /* */
    Hash,       // #
    DoubleDash, // --
    Xml,        // <!-- -->
    CssStyle,   // /* */
    Mixed,      // Multiple styles
    None,
}

pub struct LanguageDetector;

impl LanguageDetector {
    pub fn detect(path: &Path, content: Option<&str>) -> Language {
        let lang = Language::from_path(path);

        // If we couldn't detect from path and have content, try shebang
        if lang == Language::Unknown
            && let Some(content) = content
            && let Some(lang) = Self::detect_from_shebang(content)
        {
            return lang;
        }

        lang
    }

    fn detect_from_shebang(content: &str) -> Option<Language> {
        let first_line = content.lines().next()?;

        if !first_line.starts_with("#!") {
            return None;
        }

        if first_line.contains("python") {
            Some(Language::Python)
        } else if first_line.contains("node") || first_line.contains("bun") {
            Some(Language::JavaScript)
        } else if first_line.contains("ruby") {
            Some(Language::Ruby)
        } else if first_line.contains("bash") || first_line.contains("sh") {
            Some(Language::Shell)
        } else if first_line.contains("perl") {
            Some(Language::Unknown) // We don't have Perl in our enum yet
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("py"), Language::Python);
        assert_eq!(Language::from_extension("js"), Language::JavaScript);
        assert_eq!(Language::from_extension("unknown"), Language::Unknown);
    }

    #[test]
    fn test_language_from_path() {
        assert_eq!(Language::from_path(Path::new("main.rs")), Language::Rust);
        assert_eq!(Language::from_path(Path::new("app.py")), Language::Python);
        assert_eq!(
            Language::from_path(Path::new("Dockerfile")),
            Language::Shell
        );
        assert_eq!(Language::from_path(Path::new("Cargo.toml")), Language::Toml);
    }

    #[test]
    fn test_shebang_detection() {
        assert_eq!(
            LanguageDetector::detect_from_shebang("#!/usr/bin/env python3\n"),
            Some(Language::Python)
        );
        assert_eq!(
            LanguageDetector::detect_from_shebang("#!/bin/bash\n"),
            Some(Language::Shell)
        );
        assert_eq!(
            LanguageDetector::detect_from_shebang("no shebang here"),
            None
        );
    }

    #[test]
    fn test_tree_sitter_support() {
        assert!(Language::Rust.supports_tree_sitter());
        assert!(Language::Python.supports_tree_sitter());
        assert!(!Language::Unknown.supports_tree_sitter());
    }
}
