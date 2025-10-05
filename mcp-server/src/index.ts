#!/usr/bin/env node

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListResourcesRequestSchema,
  ListToolsRequestSchema,
  ReadResourceRequestSchema,
  ListPromptsRequestSchema,
  GetPromptRequestSchema,
  ErrorCode,
  McpError,
} from '@modelcontextprotocol/sdk/types.js';

import { z } from 'zod';
import { RuneBridge, RuneBridgeInstance } from './bridge.js';
import crypto from 'crypto';

// Configuration schema
const ConfigSchema = z.object({
  workspaceRoots: z.array(z.string()).default([]),
  cacheDir: z.string().default('.rune_cache'),
  maxFileSize: z.number().default(10 * 1024 * 1024),
  indexingThreads: z.number().default(4),
  enableSemantic: z.boolean().default(true),
  languages: z
    .array(z.string())
    .default(['rust', 'javascript', 'typescript', 'python', 'go', 'java', 'cpp']),
  fileWatchDebounceMs: z.number().default(500),
});

// Search query schema
const SearchQuerySchema = z.object({
  query: z.string(),
  mode: z.enum(['literal', 'regex', 'symbol', 'semantic', 'hybrid']).default('hybrid'),
  repositories: z.array(z.string()).optional(),
  filePatterns: z.array(z.string()).optional(),
  limit: z.number().default(50),
  offset: z.number().default(0),
});

class RuneMcpServer {
  private server: Server;
  public bridge: RuneBridgeInstance; // Made public for shutdown handler
  private initialized: boolean = false;

  constructor() {
    this.server = new Server(
      {
        name: 'rune-mcp-server',
        version: '0.1.0',
      },
      {
        capabilities: {
          resources: {
            list: true,
            read: true,
            subscribe: false,
          },
          tools: {
            list: true,
            call: true,
          },
          prompts: {
            list: true,
            get: true,
          },
        },
      }
    );

    this.bridge = new RuneBridge();
    this.setupHandlers();
  }

  private setupHandlers() {
    // List available tools
    this.server.setRequestHandler(ListToolsRequestSchema, async () => ({
      tools: [
        {
          name: 'search',
          description: `Advanced multi-modal code search engine for finding code patterns, implementations, and concepts across your entire codebase.

This tool provides high-performance search capabilities that go beyond simple text matching, offering intelligent code understanding through multiple search modes.

When to use this tool:
- Finding function/class/variable definitions and usages
- Locating specific code patterns or implementations
- Discovering similar code semantically (even with different syntax)
- Searching for security vulnerabilities or code smells
- Understanding code relationships and dependencies
- Finding examples of how APIs or libraries are used
- Tracking down bugs by searching for error patterns
- Refactoring by finding all instances of a pattern

Search Modes Explained:
- literal: Searches for documents containing ALL query terms (not as exact phrase). Best for single terms or function names. Avoid multiple unrelated terms.
- regex: Pattern matching with full regex support for complex searches
- symbol: AST-based search for language constructs (functions, classes, variables)
- semantic: AI-powered search understanding code meaning, not just text
- hybrid: Combines all modes using Reciprocal Rank Fusion for best results (RECOMMENDED)

Key Features:
- Typo tolerance: Automatically finds similar terms using Levenshtein distance
- Context awareness: Returns surrounding code for better understanding
- Language agnostic: Works with 100+ programming languages
- Lightning fast: Optimized with Tantivy (Rust-based Lucene) and caching
- Incremental indexing: Only re-indexes changed files

Best Practices:
1. Start with hybrid mode for most searches (default)
2. Use semantic mode for concept searches (e.g., "authentication logic")
3. Use symbol mode for finding definitions (e.g., "class UserAuth")
4. Use regex for complex patterns (e.g., "TODO.*security")
5. Apply file filters to narrow scope (e.g., filePatterns: ["*.rs", "*.go"])

Examples:
- Find authentication code: query="authentication", mode="semantic"
- Find TODO comments: query="TODO|FIXME", mode="regex"
- Find React hooks: query="use[A-Z]\\w+", mode="regex", filePatterns=["*.tsx", "*.jsx"]
- Find database connections: query="database connection pooling", mode="hybrid"
- Find similar implementations: query="quicksort algorithm", mode="semantic"
- Find specific function: query="getUserById", mode="literal" (single term works best)
- AVOID: query="mountPath /opt/kafka /data", mode="literal" (multiple unrelated terms rarely match)`,
          inputSchema: {
            type: 'object',
            properties: {
              query: {
                type: 'string',
                description:
                  'Search query. For literal mode: use single terms or exact phrases, not multiple unrelated terms',
              },
              mode: {
                type: 'string',
                enum: ['literal', 'regex', 'symbol', 'semantic', 'hybrid'],
                description: 'Search mode',
                default: 'hybrid',
              },
              repositories: {
                type: 'array',
                items: { type: 'string' },
                description: 'Filter by repositories',
              },
              filePatterns: {
                type: 'array',
                items: { type: 'string' },
                description: 'Filter by file patterns',
              },
              limit: {
                type: 'number',
                description: 'Maximum number of results',
                default: 50,
              },
              offset: {
                type: 'number',
                description: 'Offset for pagination',
                default: 0,
              },
            },
            required: ['query'],
          },
        },
        {
          name: 'index_status',
          description: `Monitor the current state of code indexing and search engine statistics.

This tool provides real-time insights into the indexing pipeline, helping you understand what has been indexed and the current search engine capacity.

When to use this tool:
- Checking if initial indexing is complete before searching
- Monitoring indexing progress for large codebases
- Debugging search issues (missing results might mean incomplete indexing)
- Understanding memory usage and cache efficiency
- Verifying that recent file changes have been indexed
- Troubleshooting performance issues

Returned Metrics:
- indexed_files: Total number of files processed and searchable
- total_symbols: Count of all extracted code symbols (functions, classes, etc.)
- index_size_bytes: Disk space used by search indices
- cache_size_bytes: Memory used by search result cache
- last_index_time: Timestamp of most recent indexing operation
- indexing_errors: Any files that failed to index

Usage Tips:
1. Always check status after starting Rune to ensure indexing is ready
2. Monitor after large refactoring to confirm re-indexing
3. Use before semantic search to verify vector database is populated
4. Check cache_size to understand memory pressure
5. Review indexing_errors to identify problematic files

Interpretation Guide:
- Low indexed_files might indicate incomplete scanning
- High cache_size suggests good performance but higher memory use
- indexing_errors often occur with binary files or very large files
- total_symbols helps gauge codebase complexity`,
          inputSchema: {
            type: 'object',
            properties: {},
          },
        },
        {
          name: 'reindex',
          description: `Force re-indexing of code repositories to update the search index with latest changes.

⚠️ IMPORTANT: Rune automatically watches your files and reindexes changes in real-time. You typically DO NOT need to use this tool during normal development. Auto-reindexing handles file creates, updates, and deletes automatically with smart debouncing.

This tool triggers a fresh scan of your codebase, updating all search indices including text, symbol, and semantic embeddings.

When to ACTUALLY use this tool (rare cases):
- Initial setup when first configuring Rune on a new codebase
- After bulk operations done outside Rune's file watching (e.g., git checkout different branch)
- When you suspect index corruption or missing files
- After changing .gitignore rules or file filters
- If auto-reindexing was disabled and you want to catch up
- To force a complete rebuild of all indices

Auto-Reindexing Features (runs automatically):
- Watches all workspace directories for changes
- Debounces rapid changes (default 500ms delay)
- Only reindexes actually changed files
- Handles file creates, modifies, and deletes
- Maintains index consistency in real-time
- No manual intervention needed

Indexing Process (when manually triggered):
1. Scans all files in workspace directories
2. Respects .gitignore and configured file filters
3. Extracts text content for literal/regex search
4. Parses AST for symbol extraction
5. Generates embeddings for semantic search (if enabled)
6. Updates all search indices atomically
7. Clears outdated cache entries

Performance Considerations:
- Auto-reindexing: Near-instant for individual file changes
- Manual full reindex: ~1000 files/second
- Semantic indexing slower due to embedding generation
- Runs in background, search remains available

Usage Examples (when needed):
- Full reindex: repositories=[]
- Specific repo: repositories=["/path/to/repo"]
- Multiple repos: repositories=["/repo1", "/repo2"]

Best Practices:
1. Trust auto-reindexing for day-to-day development
2. Only force reindex if you notice search issues
3. Use index_status to verify auto-reindexing is active
4. Manual reindex mainly useful for initial setup or recovery`,
          inputSchema: {
            type: 'object',
            properties: {
              repositories: {
                type: 'array',
                items: { type: 'string' },
                description: 'Repositories to reindex (all if empty)',
              },
            },
          },
        },
        {
          name: 'configure',
          description: `Configure and optimize the Rune search engine settings for your specific needs.

This tool allows dynamic adjustment of search engine parameters to balance performance, accuracy, and resource usage.

When to use this tool:
- Setting up Rune for a new project or workspace
- Optimizing performance for large codebases
- Enabling/disabling semantic search based on needs
- Adjusting cache settings for memory constraints
- Adding new workspace directories to search
- Customizing language-specific settings

Configuration Options:

workspaceRoots: Array of root directories to index and search
- Add all project roots for comprehensive search
- Exclude node_modules, vendor, and build directories
- Supports multiple repositories simultaneously

cacheDir: Directory for storing search indices and cache
- Use SSD for best performance
- Ensure adequate space (typically 10-20% of codebase size)
- Shared across all workspace roots

enableSemantic: Toggle AI-powered semantic search
- Requires Qdrant vector database running
- Provides concept-based search beyond text matching
- Higher memory usage but better search quality
- Disable for faster indexing on large codebases

Advanced Settings (environment variables):
- RUNE_MAX_FILE_SIZE: Skip files larger than this (default 10MB)
- RUNE_INDEXING_THREADS: Parallel indexing threads (default 4)
- RUNE_FUZZY_THRESHOLD: Typo tolerance level (default 0.75)
- RUNE_LANGUAGES: Comma-separated language list to index

Optimization Tips:
1. Start with semantic disabled for initial indexing
2. Use multiple workspace roots for monorepos
3. Increase threads for faster indexing on multicore systems
4. Adjust fuzzy threshold based on typo tolerance needs
5. Limit languages to those actually used in your project

Example Configurations:
- Single project: workspaceRoots=["/home/user/myproject"]
- Monorepo: workspaceRoots=["/repo/service-a", "/repo/service-b"]
- Memory-constrained: enableSemantic=false, cacheDir="/tmp/rune"
- Full-featured: enableSemantic=true, workspaceRoots=["."]`,
          inputSchema: {
            type: 'object',
            properties: {
              workspaceRoots: {
                type: 'array',
                items: { type: 'string' },
                description: 'Workspace root directories',
              },
              cacheDir: {
                type: 'string',
                description: 'Cache directory path',
              },
              enableSemantic: {
                type: 'boolean',
                description: 'Enable semantic search',
              },
            },
          },
        },
      ],
    }));

    // Handle tool calls
    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      try {
        switch (request.params.name) {
          case 'search': {
            const args = SearchQuerySchema.parse(request.params.arguments);
            await this.ensureInitialized();

            const searchQuery = {
              query: args.query,
              mode: args.mode,
              repositories: args.repositories?.length ? args.repositories : undefined,
              file_patterns: args.filePatterns?.length ? args.filePatterns : undefined,
              limit: args.limit || 50,
              offset: args.offset || 0,
            };

            const results = await this.bridge.search(JSON.stringify(searchQuery));

            return {
              content: [{ type: 'text', text: results }],
            };
          }

          case 'index_status': {
            await this.ensureInitialized();
            const stats = await this.bridge.getStats();
            return {
              content: [{ type: 'text', text: stats }],
            };
          }

          case 'reindex': {
            await this.ensureInitialized();
            await this.bridge.reindex();
            return {
              content: [{ type: 'text', text: 'Reindexing started' }],
            };
          }

          case 'configure': {
            const config = ConfigSchema.parse(request.params.arguments);
            await this.initializeEngine(config);
            return {
              content: [{ type: 'text', text: 'Configuration updated' }],
            };
          }

          default:
            throw new McpError(ErrorCode.MethodNotFound, `Unknown tool: ${request.params.name}`);
        }
      } catch (error) {
        if (error instanceof z.ZodError) {
          throw new McpError(
            ErrorCode.InvalidParams,
            `Invalid parameters: ${error.issues.map((e: z.ZodIssue) => e.message).join(', ')}`
          );
        }
        throw error;
      }
    });

    // List resources
    this.server.setRequestHandler(ListResourcesRequestSchema, async () => ({
      resources: [
        {
          uri: 'rune://status',
          name: 'Rune Status',
          description: 'Current status and statistics of the Rune engine',
          mimeType: 'application/json',
        },
      ],
    }));

    // Read resources
    this.server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
      if (request.params.uri === 'rune://status') {
        await this.ensureInitialized();
        const stats = await this.bridge.getStats();
        return {
          contents: [{ uri: request.params.uri, mimeType: 'application/json', text: stats }],
        };
      }

      throw new McpError(ErrorCode.InvalidRequest, `Unknown resource: ${request.params.uri}`);
    });

    // List prompts
    this.server.setRequestHandler(ListPromptsRequestSchema, async () => ({
      prompts: [
        {
          name: 'code_context',
          description: 'Get relevant code context for a query',
          arguments: [
            {
              name: 'query',
              description: 'The query to find context for',
              required: true,
            },
          ],
        },
        {
          name: 'find_definition',
          description: 'Find symbol definitions',
          arguments: [
            {
              name: 'symbol',
              description: 'The symbol name to find',
              required: true,
            },
          ],
        },
      ],
    }));

    // Get prompt
    this.server.setRequestHandler(GetPromptRequestSchema, async (request) => {
      const { name, arguments: args } = request.params;

      switch (name) {
        case 'code_context': {
          if (!args?.query) {
            throw new McpError(ErrorCode.InvalidParams, 'Query argument is required');
          }

          return {
            description: `Find relevant code context for: ${args.query}`,
            messages: [
              {
                role: 'user',
                content: {
                  type: 'text',
                  text: `Find and provide relevant code context for the following query: "${args.query}"`,
                },
              },
            ],
          };
        }

        case 'find_definition': {
          if (!args?.symbol) {
            throw new McpError(ErrorCode.InvalidParams, 'Symbol argument is required');
          }

          return {
            description: `Find definition of symbol: ${args.symbol}`,
            messages: [
              {
                role: 'user',
                content: {
                  type: 'text',
                  text: `Find the definition of the symbol: "${args.symbol}"`,
                },
              },
            ],
          };
        }

        default:
          throw new McpError(ErrorCode.InvalidRequest, `Unknown prompt: ${name}`);
      }
    });
  }

  private async ensureInitialized() {
    if (!this.initialized) {
      await this.initializeEngine();
    }
  }

  private async initializeEngine(config?: z.infer<typeof ConfigSchema>) {
    const finalConfig = config ?? this.getConfigFromEnv();

    try {
      await this.bridge.initialize(
        JSON.stringify({
          workspace_roots: finalConfig.workspaceRoots,
          cache_dir: finalConfig.cacheDir,
          max_file_size: finalConfig.maxFileSize,
          indexing_threads: finalConfig.indexingThreads,
          enable_semantic: finalConfig.enableSemantic,
          languages: finalConfig.languages,
          file_watch_debounce_ms: finalConfig.fileWatchDebounceMs,
        })
      );

      await this.bridge.start();
      this.initialized = true;
    } catch (error) {
      // Log to stderr for debugging
      console.error('Failed to initialize engine:', error);
      // Re-throw with a clean error message
      throw new McpError(
        ErrorCode.InternalError,
        `Engine initialization failed: ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }

  private getWorkspaceId(workspacePath: string): string {
    // Use crypto to create a short, unique hash of the workspace path
    const hash = crypto.createHash('sha256').update(workspacePath).digest('hex');
    // Use first 16 chars for reasonable uniqueness without being too long
    return hash.substring(0, 16);
  }

  private getConfigFromEnv(): z.infer<typeof ConfigSchema> {
    const workspaceRoots = process.env.RUNE_WORKSPACE
      ? [process.env.RUNE_WORKSPACE]
      : [process.cwd()];

    // Use RUNE_WORKSPACE_ID if set (from Docker), otherwise use the actual workspace path
    const workspaceForId = process.env.RUNE_WORKSPACE_ID ?? workspaceRoots[0];
    const workspaceId = this.getWorkspaceId(workspaceForId);
    const defaultCacheDir = process.env.RUNE_CACHE_DIR ?? '.rune_cache';

    // If running in Docker with shared cache, create workspace-specific subdirectory
    const cacheDir =
      process.env.RUNE_SHARED_CACHE === 'true'
        ? `${defaultCacheDir}/${workspaceId}`
        : defaultCacheDir;

    // Log the cache directory for debugging
    console.error(`Using cache directory: ${cacheDir} (workspace: ${workspaceForId})`);

    // Also set RUNE_WORKSPACE_ID for the Rust code to use
    if (!process.env.RUNE_WORKSPACE_ID) {
      process.env.RUNE_WORKSPACE_ID = workspaceForId;
    }

    return {
      workspaceRoots,
      cacheDir,
      maxFileSize: parseInt(process.env.RUNE_MAX_FILE_SIZE ?? '10485760'),
      indexingThreads: parseInt(process.env.RUNE_INDEXING_THREADS ?? '4'),
      enableSemantic: process.env.RUNE_ENABLE_SEMANTIC !== 'false',
      languages: process.env.RUNE_LANGUAGES?.split(',') ?? [
        'rust',
        'javascript',
        'typescript',
        'python',
        'go',
        'java',
        'cpp',
      ],
      fileWatchDebounceMs: parseInt(process.env.RUNE_FILE_WATCH_DEBOUNCE_MS ?? '500'),
    };
  }

  async run() {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);

    // Log to stderr (stdout is reserved for MCP communication)
    console.error('Rune MCP server started');
  }
}

// Filter stdout to ensure only JSON-RPC messages are sent to the MCP client
const originalStdoutWrite = process.stdout.write;
process.stdout.write = function (
  chunk: string | Uint8Array,
  ...args: Parameters<typeof process.stdout.write> extends [unknown, ...infer R] ? R : never[]
): boolean {
  // Silently drop any non-JSON writes to keep the protocol clean
  const str = chunk?.toString() ?? '';
  if (str && !str.startsWith('{') && !str.startsWith('[')) {
    // Non-JSON output is dropped to prevent protocol corruption
    return true;
  }
  return originalStdoutWrite.call(process.stdout, chunk, ...args);
} as typeof process.stdout.write;

// Main entry point
const server = new RuneMcpServer();

// Graceful shutdown handlers
const shutdown = async (signal: string) => {
  console.error(`Received ${signal}, shutting down gracefully...`);
  try {
    // Stop the bridge to ensure RocksDB is properly closed
    await server.bridge.stop();
    console.error('Bridge stopped successfully');
  } catch (error) {
    console.error('Error during shutdown:', error);
  }
  process.exit(0);
};

// Register signal handlers
process.on('SIGTERM', () => shutdown('SIGTERM'));
process.on('SIGINT', () => shutdown('SIGINT'));
process.on('SIGHUP', () => shutdown('SIGHUP'));

// Handle uncaught exceptions
process.on('uncaughtException', async (error) => {
  console.error('Uncaught exception:', error);
  await shutdown('uncaughtException');
});

// Handle unhandled promise rejections
process.on('unhandledRejection', async (reason, promise) => {
  console.error('Unhandled rejection at:', promise, 'reason:', reason);
  await shutdown('unhandledRejection');
});

server.run().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
