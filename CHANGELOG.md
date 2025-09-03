# Changelog

All notable changes to the Rune MCP Server will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [1.0.1](https://github.com/varunkamath/rune/compare/v1.0.0...v1.0.1) (2025-09-03)

### 🐛 Bug Fixes

* **ci:** Use cargo edit instead of sed ([f97b20c](https://github.com/varunkamath/rune/commit/f97b20cf0f9043b9c049c177cdccbf026b3ceee3))

## 1.0.0 (2025-09-03)

### ⚠ BREAKING CHANGES

* **ci:** First release with semantic versioning

### ✨ Features

* **ci:** add semantic-release with automated changelog generation ([5f83c7a](https://github.com/varunkamath/rune/commit/5f83c7acb57051dd9f417edd104d9cfa2d0c76f2))

### 🐛 Bug Fixes

* Add service host to container ([1c1673f](https://github.com/varunkamath/rune/commit/1c1673f4271cbb3ffca5c861691fdc6a02d6f51f))
* **ci:** Fix CI to use pnpm 10 ([1669af5](https://github.com/varunkamath/rune/commit/1669af5c4f3967a2b1f94696394a3ce87d6a9b77))
* **ci:** Fix Docker build and ensure functionality ([ca970e8](https://github.com/varunkamath/rune/commit/ca970e8595e79c58a577c497e33f3dc10054afee))
* **ci:** Fix lockfile ([a48afc4](https://github.com/varunkamath/rune/commit/a48afc4fe2000813583f125a5e98e51db7bf6640))
* Working semantic search implementation ([2802c29](https://github.com/varunkamath/rune/commit/2802c2987819f19c0fdd2aca21a4620066d6250a))
* Working standalone, tests ([870fa60](https://github.com/varunkamath/rune/commit/870fa60a90b9600df1390742e4d532ad30e00eb9))

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
