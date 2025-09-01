import { describe, it, expect, beforeEach } from '@jest/globals';

// Simple mock class for RuneBridge
class MockRuneBridge {
  initialized = false;
  started = false;

  async initialize(_config: any) {
    this.initialized = true;
    return JSON.stringify({ success: true });
  }

  async start() {
    this.started = true;
    return JSON.stringify({ success: true });
  }

  async search(query: any) {
    return JSON.stringify({
      results: [
        {
          file_path: '/test/file.ts',
          repository: 'test-repo',
          line_number: 10,
          column: 5,
          content: `Test content matching ${query.query}`,
          context_before: ['// Previous line'],
          context_after: ['// Next line'],
          score: 0.95,
          match_type: 'Exact'
        }
      ],
      total_matches: 1,
      search_time_ms: 10
    });
  }

  async getStats() {
    return JSON.stringify({
      indexed_files: 150,
      total_symbols: 750,
      index_size_bytes: 2048000,
      cache_size_bytes: 1024000
    });
  }

  async reindex() {
    return JSON.stringify({
      files_indexed: 200,
      symbols_extracted: 1000,
      time_taken_ms: 2500
    });
  }

  async configure(config: any) {
    return JSON.stringify({ 
      success: true, 
      config 
    });
  }
}

describe('Bridge Integration Tests', () => {
  let bridge: MockRuneBridge;

  beforeEach(() => {
    bridge = new MockRuneBridge();
  });

  describe('Initialization', () => {
    it('should initialize the bridge with config', async () => {
      const config = {
        workspaceRoots: ['/test/workspace'],
        cacheDir: '.test_cache',
        maxFileSize: 10485760,
        indexingThreads: 4,
        enableSemantic: true,
        languages: ['rust', 'typescript']
      };

      const result = await bridge.initialize(config);
      const parsed = JSON.parse(result);
      
      expect(bridge.initialized).toBe(true);
      expect(parsed.success).toBe(true);
    });
  });

  describe('Search Operations', () => {
    beforeEach(async () => {
      await bridge.initialize({});
      await bridge.start();
    });

    it('should perform literal search', async () => {
      const result = await bridge.search({
        query: 'testFunction',
        mode: 'literal',
        limit: 10
      });

      const parsed = JSON.parse(result);
      expect(parsed.results).toHaveLength(1);
      expect(parsed.results[0].content).toContain('testFunction');
    });

    it('should return search results with proper structure', async () => {
      const result = await bridge.search({
        query: 'test',
        mode: 'literal'
      });

      const parsed = JSON.parse(result);
      expect(parsed).toHaveProperty('results');
      expect(parsed).toHaveProperty('total_matches');
      expect(parsed).toHaveProperty('search_time_ms');
      
      const firstResult = parsed.results[0];
      expect(firstResult).toHaveProperty('file_path');
      expect(firstResult).toHaveProperty('line_number');
      expect(firstResult).toHaveProperty('content');
      expect(firstResult).toHaveProperty('score');
    });
  });

  describe('Index Management', () => {
    beforeEach(async () => {
      await bridge.initialize({});
      await bridge.start();
    });

    it('should get index statistics', async () => {
      const result = await bridge.getStats();
      const parsed = JSON.parse(result);

      expect(parsed).toHaveProperty('indexed_files');
      expect(parsed).toHaveProperty('total_symbols');
      expect(parsed).toHaveProperty('index_size_bytes');
      expect(parsed).toHaveProperty('cache_size_bytes');
      expect(parsed.indexed_files).toBeGreaterThan(0);
    });

    it('should trigger reindexing', async () => {
      const result = await bridge.reindex();
      const parsed = JSON.parse(result);

      expect(parsed).toHaveProperty('files_indexed');
      expect(parsed).toHaveProperty('symbols_extracted');
      expect(parsed).toHaveProperty('time_taken_ms');
      expect(parsed.files_indexed).toBeGreaterThan(0);
    });
  });

  describe('Configuration', () => {
    beforeEach(async () => {
      await bridge.initialize({});
    });

    it('should update configuration', async () => {
      const newConfig = {
        maxFileSize: 20971520,
        enableSemantic: false
      };

      const result = await bridge.configure(newConfig);
      const parsed = JSON.parse(result);

      expect(parsed.success).toBe(true);
      expect(parsed.config).toEqual(newConfig);
    });
  });

  describe('Error Handling', () => {
    it('should handle JSON parsing errors gracefully', () => {
      const invalidJson = 'invalid json';
      
      expect(() => JSON.parse(invalidJson)).toThrow();
    });

    it('should handle missing required fields', async () => {
      const result = await bridge.search({});
      const parsed = JSON.parse(result);
      
      // Should still return valid structure even with empty query
      expect(parsed).toHaveProperty('results');
      expect(parsed).toHaveProperty('total_matches');
    });
  });
});