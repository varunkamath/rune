# Changelog

All notable changes to the Rune MCP Server will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial release of Rune MCP Server
- Multi-modal code search (literal, regex, symbol, semantic, hybrid)
- AST-aware code chunking for 5+ languages
- Semantic search with all-MiniLM-L6-v2 embeddings
- MCP protocol implementation
- Docker support with Debian Trixie base
- Comprehensive test suite (49 Rust + 18 TypeScript tests)
- Benchmark suite for performance monitoring
- GitHub Actions CI/CD with semantic-release
