# Rune - MCP Code Context Engine

Rune is a high-performance MCP (Model Context Protocol) server that provides multi-modal code search capabilities for AI coding agents. It supports literal, regex, symbol, and semantic search across multi-repository workspaces.

## Features

- 🔍 **Multi-modal Search**: Literal, regex, symbol, semantic, and hybrid search modes
- 🚀 **High Performance**: Rust core with sub-100ms search latency
- 🌐 **Language Agnostic**: Support for 100+ programming languages via tree-sitter
- 🧠 **Semantic Understanding**: Local code embeddings with SantaCoder
- 📁 **Multi-Repository**: Search across multiple repositories simultaneously
- 🔄 **Real-time Indexing**: Automatic incremental indexing with file watching
- 🤖 **MCP Compatible**: Works with Claude Desktop and other MCP clients

## Quick Start

### Prerequisites

- Node.js 20+
- Rust 1.75+
- Docker (for Qdrant vector database)
- Git

### Installation

1. Clone the repository:
```bash
git clone https://github.com/yourusername/rune.git
cd rune
```

2. Install dependencies:
```bash
# Install Node.js dependencies
cd mcp-server
npm install

# Build Rust components
cd ..
cargo build --release
```

3. Start Qdrant:
```bash
docker-compose up -d
```

4. Build the native bridge:
```bash
cd mcp-server
npm run build:bridge
```

5. Start the MCP server:
```bash
npm run dev
```

### Testing with MCP Inspector

```bash
npx @modelcontextprotocol/inspector npm run dev
```

### Configuration for Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "rune": {
      "command": "node",
      "args": ["/absolute/path/to/rune/mcp-server/dist/index.js"],
      "env": {
        "RUNE_WORKSPACE": "/path/to/your/code",
        "RUNE_CACHE_DIR": "/path/to/cache",
        "QDRANT_URL": "http://localhost:6333"
      }
    }
  }
}
```

## Usage

### Search Tools

The MCP server provides the following tools:

#### `search`
Multi-modal code search with various modes:
- **literal**: Exact text matching
- **regex**: Regular expression patterns
- **symbol**: AST-based symbol search
- **semantic**: Embedding-based similarity search
- **hybrid**: Combined keyword and semantic search

Example:
```json
{
  "tool": "search",
  "arguments": {
    "query": "handleRequest",
    "mode": "hybrid",
    "limit": 10
  }
}
```

#### `index_status`
Get current indexing statistics and status.

#### `reindex`
Trigger manual reindexing of repositories.

#### `configure`
Update Rune configuration at runtime.

## Architecture

```
┌─────────────────────────────────────────────┐
│           MCP Client (Claude, etc)          │
└─────────────────┬───────────────────────────┘
                  │ MCP Protocol
┌─────────────────▼───────────────────────────┐
│     TypeScript MCP Interface Layer          │
└─────────────────┬───────────────────────────┘
                  │ NAPI-RS Bridge
┌─────────────────▼───────────────────────────┐
│         Rust Core Engine                    │
│  ├─ Tantivy (full-text search)             │
│  ├─ Tree-sitter (AST parsing)              │
│  ├─ Qdrant (vector database)               │
│  └─ SantaCoder (embeddings)                │
└──────────────────────────────────────────────┘
```

## Development

### Project Structure

```
rune/
├── rune-core/          # Rust core search engine
├── rune-bridge/        # NAPI-RS bindings
├── mcp-server/         # TypeScript MCP server
├── docker-compose.yml  # Qdrant setup
├── CLAUDE.md          # AI agent instructions
└── README.md          # This file
```

### Building from Source

```bash
# Build everything
cargo build --release
cd mcp-server && npm run build

# Run tests
cargo test
cd mcp-server && npm test

# Development mode
cd mcp-server && npm run dev
```

### Environment Variables

- `RUNE_WORKSPACE`: Workspace root directory to index
- `RUNE_CACHE_DIR`: Cache directory for indexes (default: `.rune_cache`)
- `RUNE_MAX_FILE_SIZE`: Maximum file size to index in bytes (default: 10MB)
- `RUNE_INDEXING_THREADS`: Number of indexing threads (default: CPU count)
- `RUNE_ENABLE_SEMANTIC`: Enable semantic search (default: true)
- `RUNE_LANGUAGES`: Comma-separated list of languages to support
- `QDRANT_URL`: Qdrant server URL (default: `http://localhost:6333`)

## Performance

Target performance metrics:
- File indexing: 1000 files/second
- Keyword search: <50ms
- Semantic search: <200ms
- Symbol lookup: <10ms
- Full reindex (10k files): <60 seconds

## Troubleshooting

### Native module not found
```bash
cd mcp-server
npm run build:bridge
```

### Qdrant connection failed
```bash
docker-compose up -d
docker-compose ps  # Check if Qdrant is running
```

### Slow indexing
- Increase `RUNE_INDEXING_THREADS`
- Check disk I/O performance
- Ensure sufficient RAM for caching

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- [MCP](https://modelcontextprotocol.io) by Anthropic
- [tree-sitter](https://tree-sitter.github.io) for AST parsing
- [Tantivy](https://github.com/quickwit-oss/tantivy) for full-text search
- [Qdrant](https://qdrant.tech) for vector storage
- [SantaCoder](https://huggingface.co/bigcode/santacoder) for code embeddings