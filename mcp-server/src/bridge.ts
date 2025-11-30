// Bridge to the Rust implementation via NAPI-RS
import { createRequire } from 'module';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));

export interface RuneBridgeInstance {
  initialize(configJson: string): Promise<void>;
  start(): Promise<void>;
  stop(): Promise<void>;
  search(queryJson: string): Promise<string>;
  getStats(): Promise<string>;
  reindex(): Promise<void>;
}

export interface RuneBridgeConstructor {
  new (): RuneBridgeInstance;
}

// Try to load the native module with various strategies
const loadNativeModule = (): RuneBridgeConstructor => {
  const possiblePaths = [
    // In mcp-server directory (built locally)
    join(__dirname, '..', 'rune.node'),
    // Platform-specific builds (for future npm distribution)
    join(__dirname, '..', `rune.${process.platform}-${process.arch}.node`),
    // Fallback paths
    join(__dirname, '..', 'rune-bridge.node'),
    join(__dirname, '..', '..', 'target', 'release', 'librune_bridge.node'),
  ];

  let lastError: Error | null = null;

  for (const modulePath of possiblePaths) {
    try {
      const nativeModule = require(modulePath);
      console.error(`Loaded native module from: ${modulePath}`);
      return nativeModule.RuneBridge;
    } catch (e) {
      lastError = e as Error;
      // Continue trying other paths
    }
  }

  // If native module loading failed, provide helpful error message
  console.error('═══════════════════════════════════════════════════════════');
  console.error('Native bridge module not found!');
  console.error('');
  console.error('To build the native module, run:');
  console.error('  pnpm run build:bridge');
  console.error('');
  console.error('Or build everything with:');
  console.error('  pnpm build');
  console.error('');
  console.error('Attempted paths:');
  possiblePaths.forEach((p) => console.error(`  - ${p}`));
  console.error('');
  console.error(`Last error: ${lastError?.message}`);
  console.error('═══════════════════════════════════════════════════════════');

  // Return mock implementation for development
  return createMockBridge();
};

// Mock implementation for development/testing when native module isn't available
const createMockBridge = (): RuneBridgeConstructor => {
  console.error('Using mock bridge implementation (search results will be empty)');

  class MockRuneBridge implements RuneBridgeInstance {
    async initialize(configJson: string): Promise<void> {
      console.error('Mock: Initializing with config:', configJson);
    }

    async start(): Promise<void> {
      console.error('Mock: Starting engine');
    }

    async stop(): Promise<void> {
      console.error('Mock: Stopping engine');
    }

    async search(queryJson: string): Promise<string> {
      console.error('Mock: Searching with query:', queryJson);
      return JSON.stringify({
        query: JSON.parse(queryJson),
        results: [],
        total_matches: 0,
        search_time_ms: 0,
      });
    }

    async getStats(): Promise<string> {
      console.error('Mock: Getting stats');
      return JSON.stringify({
        indexed_files: 0,
        total_symbols: 0,
        index_size_bytes: 0,
        cache_size_bytes: 0,
      });
    }

    async reindex(): Promise<void> {
      console.error('Mock: Reindexing');
    }
  }

  return MockRuneBridge as unknown as RuneBridgeConstructor;
};

// Load the bridge
const bridgeModule = loadNativeModule();

export const RuneBridge = bridgeModule;
