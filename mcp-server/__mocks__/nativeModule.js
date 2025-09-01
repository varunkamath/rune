// Mock for the native .node module
module.exports = {
  RuneBridge: class MockRuneBridge {
    constructor() {
      this.initialized = false;
      this.started = false;
    }

    async initialize(_config) {
      this.initialized = true;
      return JSON.stringify({ success: true });
    }

    async start() {
      this.started = true;
      return JSON.stringify({ success: true });
    }

    async search(_query) {
      // Return mock search results
      return JSON.stringify({
        results: [
          {
            file_path: '/test/file.ts',
            repository: 'test-repo',
            line_number: 10,
            column: 5,
            content: 'const testFunction = () => {}',
            context_before: ['// Previous line'],
            context_after: ['// Next line'],
            score: 0.95,
            match_type: 'Exact',
          },
        ],
        total_matches: 1,
        search_time_ms: 10,
      });
    }

    async getStats() {
      return JSON.stringify({
        indexed_files: 100,
        total_symbols: 500,
        index_size_bytes: 1024000,
        cache_size_bytes: 512000,
      });
    }

    async reindex() {
      return JSON.stringify({
        files_indexed: 100,
        symbols_extracted: 500,
        time_taken_ms: 1000,
      });
    }

    async configure(config) {
      return JSON.stringify({ success: true, config });
    }
  },
};
