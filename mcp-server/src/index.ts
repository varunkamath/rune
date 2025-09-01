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
  private bridge: RuneBridgeInstance;
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
          description: 'Search code using various modes (literal, regex, symbol, semantic, hybrid)',
          inputSchema: {
            type: 'object',
            properties: {
              query: { type: 'string', description: 'Search query' },
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
          description: 'Get indexing status and statistics',
          inputSchema: {
            type: 'object',
            properties: {},
          },
        },
        {
          name: 'reindex',
          description: 'Trigger reindexing of repositories',
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
          description: 'Configure Rune engine settings',
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

  private getConfigFromEnv(): z.infer<typeof ConfigSchema> {
    const workspaceRoots = process.env.RUNE_WORKSPACE
      ? [process.env.RUNE_WORKSPACE]
      : [process.cwd()];

    return {
      workspaceRoots,
      cacheDir: process.env.RUNE_CACHE_DIR ?? '.rune_cache',
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
    };
  }

  async run() {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);

    // Log to stderr (stdout is reserved for MCP communication)
    console.error('Rune MCP server started');
  }
}

// Debug stdout writes to find pollution source
const originalStdoutWrite = process.stdout.write;
process.stdout.write = function (
  chunk: string | Uint8Array,
  ...args: Parameters<typeof process.stdout.write> extends [unknown, ...infer R] ? R : never[]
): boolean {
  // Log any non-JSON writes to stderr for debugging
  const str = chunk?.toString() ?? '';
  if (str && !str.startsWith('{') && !str.startsWith('[')) {
    console.error('DEBUG: Non-JSON stdout write detected:', JSON.stringify(str.substring(0, 100)));
    console.error('DEBUG: Stack trace:', new Error().stack);
  }
  return originalStdoutWrite.call(process.stdout, chunk, ...args);
} as typeof process.stdout.write;

// Main entry point
const server = new RuneMcpServer();
server.run().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
