use tracing::debug;

/// Configuration for the chunking strategy
#[derive(Debug, Clone)]
pub struct ChunkerConfig {
    /// Target chunk size in characters
    pub chunk_size: usize,
    /// Overlap between chunks as a percentage (0.0 - 1.0)
    pub overlap: f32,
    /// Whether to preserve code structure (split at function/class boundaries)
    pub preserve_structure: bool,
    /// Maximum chunk size (even if preserving structure)
    pub max_chunk_size: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1500, // ~512 tokens
            overlap: 0.15,    // 15% overlap
            preserve_structure: true,
            max_chunk_size: 3000, // ~1024 tokens
        }
    }
}

/// Splits code into chunks for embedding
pub struct CodeChunker {
    config: ChunkerConfig,
}

impl CodeChunker {
    pub fn new(config: ChunkerConfig) -> Self {
        Self { config }
    }

    /// Chunk a file's content intelligently
    pub fn chunk_file(&self, content: &str, file_path: &str) -> Vec<CodeChunk> {
        if content.is_empty() {
            return Vec::new();
        }

        // Detect language from file extension
        let language = Self::detect_language(file_path);

        if self.config.preserve_structure && language.is_some() {
            self.chunk_with_structure(content, file_path, language.as_deref())
        } else {
            self.chunk_simple(content, file_path)
        }
    }

    /// Simple chunking with fixed size and overlap
    fn chunk_simple(&self, content: &str, file_path: &str) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();

        let chunk_lines = self.config.chunk_size / 80; // Assume ~80 chars per line
        let overlap_lines = (chunk_lines as f32 * self.config.overlap) as usize;

        let mut start_line = 0;
        while start_line < lines.len() {
            let end_line = (start_line + chunk_lines).min(lines.len());

            let chunk_content = lines[start_line..end_line].join("\n");

            chunks.push(CodeChunk {
                content: chunk_content,
                file_path: file_path.to_string(),
                start_line: start_line + 1, // 1-indexed
                end_line,
                language: Self::detect_language(file_path),
                chunk_type: ChunkType::Block,
            });

            // Move to next chunk with overlap
            if end_line >= lines.len() {
                break;
            }
            start_line = end_line - overlap_lines;
        }

        debug!("Created {} simple chunks for {}", chunks.len(), file_path);
        chunks
    }

    /// AST-aware chunking that respects code structure
    fn chunk_with_structure(
        &self,
        content: &str,
        file_path: &str,
        language: Option<&str>,
    ) -> Vec<CodeChunk> {
        // For now, fall back to heuristic-based structural chunking
        // In the future, we can use tree-sitter for proper AST parsing
        self.chunk_with_heuristics(content, file_path, language)
    }

    /// Heuristic-based structural chunking
    fn chunk_with_heuristics(
        &self,
        content: &str,
        file_path: &str,
        language: Option<&str>,
    ) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();
        let mut current_chunk = Vec::new();
        let mut current_size = 0;
        let mut chunk_start = 0;
        let mut in_function = false;
        let mut brace_depth = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect function/class boundaries based on language
            let is_boundary = match language {
                Some("rust") => {
                    trimmed.starts_with("pub fn")
                        || trimmed.starts_with("fn ")
                        || trimmed.starts_with("pub struct")
                        || trimmed.starts_with("struct ")
                        || trimmed.starts_with("impl ")
                        || trimmed.starts_with("pub trait")
                        || trimmed.starts_with("trait ")
                },
                Some("python") => {
                    trimmed.starts_with("def ")
                        || trimmed.starts_with("class ")
                        || trimmed.starts_with("async def ")
                },
                Some("javascript") | Some("typescript") => {
                    trimmed.starts_with("function ")
                        || trimmed.starts_with("const ")
                        || trimmed.starts_with("let ")
                        || trimmed.starts_with("class ")
                        || trimmed.starts_with("export ")
                },
                _ => false,
            };

            // Track brace depth for languages that use braces
            if matches!(
                language,
                Some("rust") | Some("javascript") | Some("typescript") | Some("java") | Some("cpp")
            ) {
                brace_depth += line.chars().filter(|&c| c == '{').count() as i32;
                brace_depth -= line.chars().filter(|&c| c == '}').count() as i32;
            }

            // Check if we should start a new chunk
            let should_split = is_boundary
                && !current_chunk.is_empty()
                && (current_size > self.config.chunk_size / 2 || brace_depth == 0);

            if should_split || current_size > self.config.max_chunk_size {
                // Save current chunk
                if !current_chunk.is_empty() {
                    chunks.push(CodeChunk {
                        content: current_chunk.join("\n"),
                        file_path: file_path.to_string(),
                        start_line: chunk_start + 1,
                        end_line: i,
                        language: language.map(|s| s.to_string()),
                        chunk_type: if in_function {
                            ChunkType::Function
                        } else {
                            ChunkType::Block
                        },
                    });

                    // Add overlap if not at a clean boundary
                    if !is_boundary && self.config.overlap > 0.0 {
                        let overlap_lines =
                            (current_chunk.len() as f32 * self.config.overlap) as usize;
                        let overlap_start = current_chunk.len().saturating_sub(overlap_lines);
                        current_chunk = current_chunk[overlap_start..].to_vec();
                        chunk_start = i - current_chunk.len();
                        current_size = current_chunk.iter().map(|l: &&str| l.len()).sum::<usize>();
                    } else {
                        current_chunk.clear();
                        chunk_start = i;
                        current_size = 0;
                    }
                }

                in_function = is_boundary && trimmed.contains("fn ") || trimmed.contains("def ");
            }

            current_chunk.push(line);
            current_size += line.len();
        }

        // Add final chunk
        if !current_chunk.is_empty() {
            chunks.push(CodeChunk {
                content: current_chunk.join("\n"),
                file_path: file_path.to_string(),
                start_line: chunk_start + 1,
                end_line: lines.len(),
                language: language.map(|s| s.to_string()),
                chunk_type: if in_function {
                    ChunkType::Function
                } else {
                    ChunkType::Block
                },
            });
        }

        debug!(
            "Created {} structural chunks for {}",
            chunks.len(),
            file_path
        );
        chunks
    }

    /// Detect language from file extension
    fn detect_language(file_path: &str) -> Option<String> {
        let extension = file_path.rsplit('.').next()?;

        let language = match extension {
            "rs" => "rust",
            "py" => "python",
            "js" | "mjs" => "javascript",
            "ts" | "tsx" => "typescript",
            "java" => "java",
            "cpp" | "cc" | "cxx" | "hpp" | "h" => "cpp",
            "go" => "go",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "kt" => "kotlin",
            "scala" => "scala",
            "sh" | "bash" => "bash",
            "sql" => "sql",
            "md" => "markdown",
            _ => return None,
        };

        Some(language.to_string())
    }
}

/// Represents a chunk of code
#[derive(Debug, Clone)]
pub struct CodeChunk {
    pub content: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: Option<String>,
    pub chunk_type: ChunkType,
}

/// Type of code chunk
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkType {
    Function,
    Class,
    Block,
    Import,
    Documentation,
}
