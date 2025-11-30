# Rune

Rune is an MCP (Model Context Protocol) server that provides code search
capabilities for AI coding agents. It combines full-text search, AST-based
symbol extraction, and vector similarity search into a unified interface
accessible through the MCP protocol.

## Overview

Rune indexes codebases and exposes search functionality through MCP tools:

- **search_symbols**: AST-based search for code symbols (functions, classes,
  methods, etc.)
- **search_semantic**: AI-powered semantic search using embeddings
- **index_status**: Query indexing statistics and engine state
- **reindex**: Trigger manual re-indexing of repositories
- **configure**: Adjust engine settings at runtime

Note: For text search and regex patterns, use terminal tools like `ripgrep` (rg)
which agents can access directly.

The server is built with a Rust core for search operations, connected to a
TypeScript MCP interface via NAPI-RS bindings.

## Architecture

```text
AI Agent (Claude, Copilot, etc.)
    |
    | MCP Protocol (JSON-RPC 2.0 over stdio)
    v
TypeScript MCP Server (index.ts)
    |
    | NAPI-RS Bridge
    v
Rust Core Engine
    |
    +-- Tantivy (full-text indexing)
    +-- Tree-sitter (AST parsing, 16 languages)
    +-- RocksDB (file metadata)
    +-- Qdrant (vector storage for semantic search)
    +-- ONNX Runtime (embeddings via all-MiniLM-L6-v2)
```

## Installation

### Docker

```bash
docker build -t rune-mcp:latest .

docker run -d \
  --name rune \
  -v ~/Projects:/workspace:ro \
  -v ~/.rune:/data \
  rune-mcp:latest
```

### From Source

Requires Rust and Node.js 22+.

```bash
pnpm install
cargo build --release
pnpm build
```

## IDE Configuration

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS)
or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "rune": {
      "command": "docker",
      "args": [
        "run",
        "--rm",
        "-i",
        "-v",
        "${HOME}:/workspace:ro",
        "-v",
        "${HOME}/.rune:/data",
        "rune-mcp:latest",
        "node",
        "/app/dist/index.js"
      ]
    }
  }
}
```

### Claude Code

Create `.claude/mcp.json` in your project:

```json
{
  "mcpServers": {
    "rune": {
      "command": "docker",
      "args": [
        "run",
        "--rm",
        "-i",
        "-v",
        "${PWD}:/workspace:ro",
        "-v",
        "${HOME}/.rune:/data",
        "rune-mcp:latest",
        "node",
        "/app/dist/index.js"
      ]
    }
  }
}
```

### VS Code with GitHub Copilot

Add to `.vscode/settings.json`:

```json
{
  "github.copilot.mcp.servers": {
    "rune": {
      "command": "docker",
      "args": [
        "run",
        "--rm",
        "-i",
        "-v",
        "${workspaceFolder}:/workspace:ro",
        "-v",
        "${HOME}/.rune:/data",
        "rune-mcp:latest",
        "node",
        "/app/dist/index.js"
      ]
    }
  }
}
```

### Cursor

Add to `.cursor/mcp.json`:

```json
{
  "servers": {
    "rune": {
      "command": "docker",
      "args": [
        "run",
        "--rm",
        "-i",
        "-v",
        "${workspaceFolder}:/workspace:ro",
        "-v",
        "${HOME}/.rune:/data",
        "rune-mcp:latest",
        "node",
        "/app/dist/index.js"
      ]
    }
  }
}
```

## Search Modes

### Symbol (search_symbols)

AST-based search targeting language constructs. Uses tree-sitter to parse code
and extract functions, classes, structs, enums, traits, methods, and other
definitions. Best for finding where symbols are defined.

### Semantic (search_semantic)

Vector similarity search using embeddings. Queries are embedded with
all-MiniLM-L6-v2 (384 dimensions) and matched against code chunks stored in
Qdrant. Use natural language queries to find code by meaning, not just text.

### Text Search Alternative

For literal text search or regex patterns, use terminal tools directly:

```bash
# Find exact text
rg "handleRequest" --type ts

# Pattern matching
rg "TODO.*security" --type rust

# Find function calls
rg "getUserById\(" --type js
```

Agents can execute these commands through their bash/terminal tools.

## Supported Languages

Tree-sitter parsers provide AST-aware indexing for:

- Rust
- JavaScript
- TypeScript
- Python
- Go
- Java
- C/C++
- C#
- Ruby
- PHP
- HTML
- CSS
- JSON
- YAML
- TOML

Additional languages are detected by file extension but indexed as plain text.

## Configuration

### Environment Variables

| Variable                      | Default                 | Description                        |
| ----------------------------- | ----------------------- | ---------------------------------- |
| `RUNE_WORKSPACE`              | Current directory       | Root directory to index            |
| `RUNE_CACHE_DIR`              | `.rune_cache`           | Directory for indices and metadata |
| `RUNE_MAX_FILE_SIZE`          | `10485760`              | Max file size in bytes (10MB)      |
| `RUNE_INDEXING_THREADS`       | `4`                     | Parallel indexing threads          |
| `RUNE_ENABLE_SEMANTIC`        | `true`                  | Enable semantic search             |
| `RUNE_LANGUAGES`              | (see below)             | Languages to index                 |
| `RUNE_FILE_WATCH_DEBOUNCE_MS` | `500`                   | File watcher debounce (ms)         |
| `RUNE_QUANTIZATION_MODE`      | `scalar`                | Vector quantization mode           |
| `QDRANT_URL`                  | `http://localhost:6334` | Qdrant gRPC endpoint               |

Default languages: `rust,javascript,typescript,python,go,java,cpp`

Quantization options: `none`, `scalar`, `binary`, `asymmetric`

### Quantization Modes

Vector quantization reduces memory usage for semantic search:

- **none**: Full float32 precision (1536 bytes per vector)
- **scalar**: int8 quantization (384 bytes per vector, 75% reduction)
- **binary**: 1-bit quantization (48 bytes per vector, 97% reduction)
- **asymmetric**: Binary storage with scalar queries (48 bytes per vector)

### Multi-Agent Cache Isolation

When running multiple agents on different projects, enable cache isolation to
prevent lock conflicts:

```bash
docker run --rm -i \
  -v "${PWD}:/workspace:ro" \
  -v "${HOME}/.rune:/data" \
  -e "RUNE_SHARED_CACHE=true" \
  -e "RUNE_WORKSPACE_ID=${PWD}" \
  rune-mcp:latest
```

Each workspace receives a separate cache directory based on a SHA256 hash of its
path.

## MCP Tools

### search_symbols

Find code symbol definitions:

```json
{
  "tool": "search_symbols",
  "arguments": {
    "query": "handleRequest",
    "limit": 50,
    "offset": 0,
    "filePatterns": ["*.ts"]
  }
}
```

### search_semantic

Find code by meaning using natural language:

```json
{
  "tool": "search_semantic",
  "arguments": {
    "query": "authentication and login handling",
    "limit": 50,
    "offset": 0,
    "filePatterns": ["*.py", "*.ts"]
  }
}
```

### index_status

```json
{
  "tool": "index_status"
}
```

Returns indexed file count, symbol count, cache size, and file watcher status.

### reindex

```json
{
  "tool": "reindex",
  "arguments": {
    "repositories": ["specific-repo"]
  }
}
```

Manual reindexing is typically unnecessary; the file watcher handles changes
automatically.

### configure

```json
{
  "tool": "configure",
  "arguments": {
    "workspaceRoots": ["/path/to/project"],
    "enableSemantic": true
  }
}
```

## Storage

- **Tantivy**: Full-text index with fields for path, content, language, symbols,
  and repository
- **RocksDB**: File metadata including path, size, modification time, Blake3
  hash, and indexing timestamp
- **Qdrant**: Vector embeddings for semantic search with workspace-isolated
  collections

## Caching

Multi-tier cache architecture:

- **L1**: In-memory DashMap with 10,000 entry capacity and 5-minute TTL
- **L2**: RocksDB for persistent metadata
- **L3**: Qdrant for vector storage

Queries shorter than 2 characters bypass caching.

## File Watching

Rune monitors the workspace for changes using the `notify` crate with
configurable debouncing. File events (create, modify, delete) trigger
incremental reindexing.

Blake3 content hashing enables efficient change detection: files with unchanged
hashes skip reindexing and only update their timestamp.

## Development

```bash
# Build everything
pnpm build

# Run with MCP Inspector
pnpm inspect

# Run tests
cargo test
pnpm test

# Run benchmarks
cargo bench
```

## Troubleshooting

### Container Logs

```bash
docker logs rune
```

### Reset Cache

```bash
docker stop rune && docker rm rune
rm -rf ~/.rune
```

### Semantic Search Not Working

Qdrant must be running and accessible at the configured URL (default:
`http://localhost:6334`). The Docker image includes an embedded Qdrant instance.

## License

MIT

## Acknowledgments

- [Model Context Protocol](https://modelcontextprotocol.io) by Anthropic
- [Tree-sitter](https://tree-sitter.github.io) for AST parsing
- [Tantivy](https://github.com/quickwit-oss/tantivy) for full-text search
- [Qdrant](https://qdrant.tech) for vector storage
- [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2)
  for embeddings
