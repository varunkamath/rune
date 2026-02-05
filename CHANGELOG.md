# Changelog

All notable changes to the Rune MCP Server will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [1.2.3](https://github.com/varunkamath/rune/compare/v1.2.2...v1.2.3) (2026-02-05)

### 🐛 Bug Fixes

* Cleanup dead code ([4c61ec8](https://github.com/varunkamath/rune/commit/4c61ec8519cc1221c82417c7ad18d8a6bd132bbe))
* Improve code robustness with proper error handling, remove dead code, and fix inaccurate language support claims ([3fd7d84](https://github.com/varunkamath/rune/commit/3fd7d845c4c8520c9732d1da19a3586b9337e281))
* Remove overlap features ([b1c3c2b](https://github.com/varunkamath/rune/commit/b1c3c2bb4cb51f3bb1e64a9e81268a8f63447efc))

## [1.2.2](https://github.com/varunkamath/rune/compare/v1.2.1...v1.2.2) (2025-11-27)

### 🐛 Bug Fixes

* Migrate to oxlint ([28730f2](https://github.com/varunkamath/rune/commit/28730f2f9a75013bd7f1b15a2b1e93d9326c6528))

## [1.2.1](https://github.com/varunkamath/rune/compare/v1.2.0...v1.2.1) (2025-11-27)

### 🐛 Bug Fixes

* Fix several bugs ([22f64e5](https://github.com/varunkamath/rune/commit/22f64e5de794a2fb9f9c7b71b2513ffdcf01a68e))
* Update dependencies and fix failing tests ([a026e4f](https://github.com/varunkamath/rune/commit/a026e4f56d00ec626d381e5baa10179dd6497f57))

## [1.2.0](https://github.com/varunkamath/rune/compare/v1.1.0...v1.2.0) (2025-10-05)

### ✨ Features

* Add incremental hash checking, dead code allows ([7a5eb56](https://github.com/varunkamath/rune/commit/7a5eb56dd8e4bfd90485e68079849b48334e9dbf))
* Multi-word search ([24f1d5e](https://github.com/varunkamath/rune/commit/24f1d5ed287259dc885f6e4ead3bc8302b486f8c))
* Workspace dynamic caching ([4d09ecf](https://github.com/varunkamath/rune/commit/4d09ecf167a5c3b1fbb6d03520ee046ede9ca601))

### 🐛 Bug Fixes

* **ci:** Fix docker build process ([4fe2bb1](https://github.com/varunkamath/rune/commit/4fe2bb19de07f805a8b650d1735b1cfa3896305a))
* Fix build errors, incremental autoindex ([c1dcfe3](https://github.com/varunkamath/rune/commit/c1dcfe381f132de5ee0ac192a1d705bacdcf8ef7))
* Remove error-prone HTTP fallback ([8d4b217](https://github.com/varunkamath/rune/commit/8d4b2179ac44ba310c12840bdf3121b0f1946098))

### 📚 Documentation

* Clarify literal tool usage ([6ca51dc](https://github.com/varunkamath/rune/commit/6ca51dcc5af0667ec402f029f79f1e7caf633b4e))
* Improve tool descriptions ([0bc4459](https://github.com/varunkamath/rune/commit/0bc4459155d43f796278c02e0aea9ac0a552e7f6))

## [1.1.0](https://github.com/varunkamath/rune/compare/v1.0.4...v1.1.0) (2025-09-10)

### ✨ Features

* Fuzzy matching, caching ([9a9d2f7](https://github.com/varunkamath/rune/commit/9a9d2f7b9c660b7decd1684fb2547db4d61ef61f))

### 📚 Documentation

* Fix config JSON ([affe2de](https://github.com/varunkamath/rune/commit/affe2dea5a823df40bccfe947205d0763fee8a8d))

### ✅ Tests

* Fix literal test with fuzzy enabled ([79e6a50](https://github.com/varunkamath/rune/commit/79e6a50bb810dc105a39ddad9cbe4e9c40f68f0f))

## [1.0.4](https://github.com/varunkamath/rune/compare/v1.0.3...v1.0.4) (2025-09-05)

### 🐛 Bug Fixes

* Add quantization ([cfce844](https://github.com/varunkamath/rune/commit/cfce844ab9ee74aaf777c8dd62d4d4b4e43bd2e7))

## [1.0.3](https://github.com/varunkamath/rune/compare/v1.0.2...v1.0.3) (2025-09-04)

### 🐛 Bug Fixes

* Fix benchmarks tokio issues ([93a51fc](https://github.com/varunkamath/rune/commit/93a51fc3cd46dc5d2425675dc033dcd0f5501dc1))

## [1.0.2](https://github.com/varunkamath/rune/compare/v1.0.1...v1.0.2) (2025-09-03)

### 🐛 Bug Fixes

* **ci:** Include root Cargo.toml ([47106cc](https://github.com/varunkamath/rune/commit/47106cc93932c5a7d0baaacb75accb59988049b3))

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
