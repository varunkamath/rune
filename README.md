# Rune - High-Performance MCP Code Context Engine

Rune is a blazing-fast MCP (Model Context Protocol) server that provides
multi-modal code search capabilities for AI coding agents. With embedded Qdrant
vector database, it delivers literal, regex, symbol, semantic, and hybrid search
across multi-repository workspaces.

## 🚀 Quick Start

### 🐳 Docker Deployment (Production Ready)

```bash
# Build the Docker image
docker build -t rune-mcp:latest .

# Start Rune with embedded Qdrant
docker run -d \
  --name rune \
  -v ~/Projects:/workspace:ro \
  -v ~/.rune:/data \
  -p 6333:6333 \
  -p 6334:6334 \
  rune-mcp:latest
```

**Note**: The container includes both Rune MCP server and Qdrant vector
database, managed by s6-overlay for process supervision.

### Verify Installation

```bash
# Check container status
docker logs rune --tail 50

# Verify Qdrant is running
curl http://localhost:6333/
# Should return: {"title":"qdrant - vector search engine","version":"1.15.4",...}
```

## 📦 Installation & Setup

### For Claude Desktop

Add to Claude Desktop configuration:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`  
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

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
        "${HOME}/Projects:/workspace:ro",
        "-v",
        "${HOME}/.rune:/data",
        "rune-mcp:latest",
        "node",
        "/app/dist/index.js"
      ],
      "env": {}
    }
  }
}
```

**Note**: The container will start automatically when Claude Desktop connects.
Use `${HOME}` for cross-platform compatibility.

**Restart Claude Desktop** to activate Rune

### For Claude Code

Create or edit `.claude/mcp.json` in your project root:

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
      ],
      "env": {}
    }
  }
}
```

**Note**: The container starts automatically when Claude Code connects. It
indexes your current project directory.

### For VS Code with GitHub Copilot

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

### For Cursor IDE

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

### For Continue.dev (Agent Mode)

Create `.continue/mcpServers/rune.json`:

```json
{
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
  ],
  "transport": "stdio"
}
```

### For Windsurf IDE

Add to `~/.codeium/windsurf/mcp_config.json`:

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
        "${HOME}/Projects:/workspace:ro",
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

## 🎯 Features

- **🔍 Multi-modal Search**: Literal, regex, symbol, semantic, and hybrid search
  modes
- **🚀 High Performance**: Rust core with sub-50ms search latency
- **🌐 Language Agnostic**: Support for 100+ programming languages via
  tree-sitter
- **🧠 Semantic Understanding**: Real embeddings with all-MiniLM-L6-v2
- **📁 Multi-Repository**: Search across multiple repositories simultaneously
- **🔄 Real-time Indexing**: Automatic incremental indexing with file watching
- **🐳 All-in-One Container**: Rune + Qdrant in a single Docker container
- **🤖 MCP Compatible**: Works with Claude, Copilot, Cursor, and more

## 🛠️ Advanced Configuration

### Custom Workspace Paths

```bash
docker run -d \
  --name rune \
  -v /path/to/project1:/workspace/project1:ro \
  -v /path/to/project2:/workspace/project2:ro \
  -v ~/.rune:/data \
  -e RUNE_INDEXING_THREADS=8 \
  rune-mcp:latest
```

### Using External Qdrant

```bash
docker run -d \
  --name rune \
  -v ~/Projects:/workspace:ro \
  -e QDRANT_URL=http://your-qdrant:6334 \
  rune-mcp:latest
```

### Docker Compose

```yaml
services:
  rune:
    build: .
    image: rune-mcp:latest
    container_name: rune
    volumes:
      - ${HOME}/Projects:/workspace:ro
      - ~/.rune:/data
    environment:
      - RUNE_INDEXING_THREADS=4
      - RUNE_MAX_FILE_SIZE=10485760
    restart: unless-stopped
```

## 🔧 MCP Tools Available

### `search`

Multi-modal code search with various modes:

```json
{
  "tool": "search",
  "arguments": {
    "query": "handleRequest",
    "mode": "hybrid",
    "limit": 10,
    "file_pattern": "*.ts",
    "repositories": ["repo1", "repo2"]
  }
}
```

**Search Modes:**

- `literal`: Exact text matching
- `regex`: Regular expression patterns
- `symbol`: AST-based symbol search (functions, classes, etc.)
- `semantic`: Embedding-based similarity search
- `hybrid`: Combined keyword and semantic search (best results)

### `index_status`

Get current indexing statistics:

```json
{
  "tool": "index_status"
}
```

Returns file count, symbol count, cache size, and indexing progress.

### `reindex`

Trigger manual reindexing:

```json
{
  "tool": "reindex",
  "arguments": {
    "repositories": ["specific-repo"] // Optional, reindexes all if omitted
  }
}
```

### `configure`

Update configuration at runtime:

```json
{
  "tool": "configure",
  "arguments": {
    "max_file_size": 20971520,
    "indexing_threads": 8
  }
}
```

## 🐛 Troubleshooting

### Check Container Status

```bash
# View logs
docker logs rune

# Check health
docker exec rune curl http://localhost:3333/health

# View indexing status
docker exec rune curl http://localhost:3333/status
```

### View Configuration Templates

```bash
# List available templates
docker exec rune ls /config/

# Get specific configuration
docker exec rune cat /config/claude-desktop.json
```

### Reset and Restart

```bash
# Stop and remove container
docker stop rune && docker rm rune

# Clear cache (optional)
rm -rf ~/.rune

# Start fresh
docker run -d --name rune -v ~/Projects:/workspace:ro rune-mcp:latest
```

### Common Issues

**Container won't start:**

- Check if port 3333 is already in use
- Ensure Docker has enough resources (2GB RAM minimum)
- Verify volume mount paths exist

**Slow indexing:**

- Increase `RUNE_INDEXING_THREADS` environment variable
- Check disk I/O performance
- Exclude large binary files or node_modules

**Semantic search not working:**

- Qdrant may still be initializing (wait 30 seconds)
- Check Qdrant health: `docker exec rune curl http://localhost:6334/health`
- Verify `RUNE_ENABLE_SEMANTIC=true` is set

## 📊 Performance

- **Indexing**: ~1000 files/second
- **Literal Search**: <10ms
- **Regex Search**: <50ms
- **Symbol Search**: <20ms
- **Semantic Search**: <200ms
- **Hybrid Search**: <250ms
- **Memory Usage**: 512MB-2GB (depending on workspace size)
- **Container Size**: ~400MB

## 🏗️ Architecture

```text
┌─────────────────────────────────────────────┐
│           AI Coding Agent (Claude, etc)      │
└─────────────────┬───────────────────────────┘
                  │ MCP Protocol (JSON-RPC)
┌─────────────────▼───────────────────────────┐
│              Docker Container                │
│  ┌─────────────────────────────────────┐    │
│  │     Rune MCP Server (Node.js)       │    │
│  └──────────────┬──────────────────────┘    │
│                 │ NAPI Bridge               │
│  ┌──────────────▼──────────────────────┐    │
│  │     Rust Core Engine                │    │
│  │  • Tantivy (full-text search)       │    │
│  │  • Tree-sitter (AST parsing)        │    │
│  │  • ONNX Runtime (embeddings)        │    │
│  └──────────────┬──────────────────────┘    │
│                 │                           │
│  ┌──────────────▼──────────────────────┐    │
│  │     Qdrant Vector Database          │    │
│  │     (Embedded in container)         │    │
│  └──────────────────────────────────────┘    │
└──────────────────────────────────────────────┘
```

## 🔒 Security

- ✅ Runs as non-root user (uid 1001)
- ✅ Read-only workspace mount recommended
- ✅ SBOM available for vulnerability scanning
- ✅ Signed with Cosign/Sigstore
- ✅ No network access required (except MCP)
- ✅ Isolated data directory

## 🧑‍💻 Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/rune-mcp/server.git
cd rune

# Build Docker image
docker build -t rune-mcp:local .

# Or build locally (requires Rust 1.89 + Node.js 22)
cargo build --release
cd mcp-server && npm install && npm run build
```

### Running Tests

```bash
# Rust tests
cargo test

# TypeScript tests
cd mcp-server && npm test

# Benchmarks
cargo bench
```

### Environment Variables

| Variable                | Description                | Default                         |
| ----------------------- | -------------------------- | ------------------------------- |
| `RUNE_WORKSPACE`        | Workspace root to index    | `/workspace`                    |
| `RUNE_CACHE_DIR`        | Cache directory            | `/data/cache`                   |
| `RUNE_MAX_FILE_SIZE`    | Max file size in bytes     | `10485760` (10MB)               |
| `RUNE_INDEXING_THREADS` | Number of indexing threads | CPU count                       |
| `RUNE_ENABLE_SEMANTIC`  | Enable semantic search     | `true`                          |
| `RUNE_LANGUAGES`        | Comma-separated languages  | `rust,js,ts,python,go,java,cpp` |
| `QDRANT_URL`            | Qdrant server URL          | `http://localhost:6334`         |

## 📝 License

MIT License - see [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

- [MCP](https://modelcontextprotocol.io) by Anthropic
- [Tree-sitter](https://tree-sitter.github.io) for AST parsing
- [Tantivy](https://github.com/quickwit-oss/tantivy) for full-text search
- [Qdrant](https://qdrant.tech) for vector storage
- [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2)
  for embeddings

## 🤝 Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md)
for details.

## 📬 Support

- **Issues**: [GitHub Issues](https://github.com/rune-mcp/server/issues)
- **Discussions**:
  [GitHub Discussions](https://github.com/rune-mcp/server/discussions)
- **Security**: Report vulnerabilities via GitHub Security Advisories

---

**Ready to supercharge your AI coding workflow?** Get started with one command:

```bash
docker run -d --name rune -v ~/Projects:/workspace:ro ghcr.io/rune-mcp/server:latest
```
