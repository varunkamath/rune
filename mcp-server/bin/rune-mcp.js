#!/usr/bin/env node

import { spawn, execSync } from 'child_process';
import { existsSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import process from 'process';

const __dirname = dirname(fileURLToPath(import.meta.url));

// Check if Docker is available
async function checkDocker() {
  try {
    execSync('docker --version', { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

// Check if Docker container is running
async function checkDockerContainer() {
  try {
    execSync('docker ps --filter name=rune --format "{{.Names}}"', { stdio: 'pipe' });
    return true;
  } catch {
    return false;
  }
}

// Parse CLI arguments
function parseArgs() {
  const args = process.argv.slice(2);
  const options = {
    help: false,
    version: false,
    workspace: process.cwd(),
    docker: false,
    noQdrant: false,
    qdrantUrl: null,
    cacheDir: null,
  };

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case '-h':
      case '--help':
        options.help = true;
        break;
      case '-v':
      case '--version':
        options.version = true;
        break;
      case '--workspace':
        options.workspace = args[++i] || process.cwd();
        break;
      case '--docker':
        options.docker = true;
        break;
      case '--no-qdrant':
        options.noQdrant = true;
        break;
      case '--qdrant-url':
        options.qdrantUrl = args[++i];
        break;
      case '--cache-dir':
        options.cacheDir = args[++i];
        break;
    }
  }

  return options;
}

// Show help message
function showHelp() {
  console.log(`
Rune MCP Server - High-performance code context engine

Usage: rune-mcp [options]

Options:
  -h, --help              Show this help message
  -v, --version           Show version information
  --workspace <path>      Set workspace directory (default: current directory)
  --docker                Prefer Docker execution if available
  --no-qdrant            Skip Qdrant setup (disables semantic search)
  --qdrant-url <url>      Use external Qdrant instance
  --cache-dir <path>      Set cache directory

Quick Start:
  npx -y @rune-mcp/latest           # Run with automatic setup
  
Docker (Recommended):
  docker run -d --name rune -v \$(pwd):/workspace:ro ghcr.io/rune-mcp/server:latest

Examples:
  rune-mcp --workspace ~/Projects
  rune-mcp --qdrant-url http://remote:6334
  rune-mcp --docker

For more information, visit: https://github.com/rune-mcp/server
`);
}

// Show version
function showVersion() {
  try {
    const packageJson = require('../package.json');
    console.log(`Rune MCP Server v${packageJson.version}`);
  } catch {
    console.log('Rune MCP Server');
  }
}

// Start Docker container
async function startDockerContainer(workspace) {
  console.log('🐳 Starting Rune in Docker container...');
  
  try {
    const result = execSync(
      `docker run -d --name rune -v "${workspace}:/workspace:ro" -v ~/.rune:/data ghcr.io/rune-mcp/server:latest`,
      { stdio: 'pipe', encoding: 'utf8' }
    );
    
    console.log('✅ Rune container started successfully!');
    console.log('');
    console.log('Add to your IDE configuration:');
    console.log('');
    console.log('For Claude Desktop:');
    console.log(JSON.stringify({
      mcpServers: {
        rune: {
          command: "docker",
          args: ["exec", "-i", "rune", "node", "/app/dist/index.js"],
          env: {}
        }
      }
    }, null, 2));
    
    return true;
  } catch (error) {
    console.error('❌ Failed to start Docker container:', error.message);
    return false;
  }
}

// Main function
async function main() {
  const options = parseArgs();

  if (options.help) {
    showHelp();
    process.exit(0);
  }

  if (options.version) {
    showVersion();
    process.exit(0);
  }

  // Check if Docker is available and suggest using it
  if (!options.docker && await checkDocker()) {
    console.log('🐳 Docker detected! For the best experience, we recommend running Rune in Docker:');
    console.log('');
    console.log(`  docker run -d --name rune -v "${options.workspace}:/workspace:ro" ghcr.io/rune-mcp/server:latest`);
    console.log('');
    console.log('Or run with --docker flag to use Docker automatically:');
    console.log('  rune-mcp --docker');
    console.log('');
    console.log('Continuing with local execution...');
    console.log('');
  }

  // If Docker flag is set, try to use Docker
  if (options.docker) {
    if (await checkDocker()) {
      if (await checkDockerContainer()) {
        console.log('✅ Rune container is already running');
      } else {
        await startDockerContainer(options.workspace);
      }
      process.exit(0);
    } else {
      console.error('❌ Docker is not available. Please install Docker first.');
      console.log('Visit: https://docs.docker.com/get-docker/');
      process.exit(1);
    }
  }

  // Check for native module
  const nativeModule = join(__dirname, '..', 'rune.node');
  if (!existsSync(nativeModule)) {
    console.error('⚠️  Native module not found.');
    console.error('');
    console.error('Please either:');
    console.error('1. Use Docker (recommended): docker run -d --name rune -v $(pwd):/workspace:ro ghcr.io/rune-mcp/server:latest');
    console.error('2. Build from source: npm run build:bridge');
    process.exit(1);
  }

  // Set up environment
  const env = {
    ...process.env,
    RUNE_WORKSPACE: options.workspace,
  };

  if (options.qdrantUrl) {
    env.QDRANT_URL = options.qdrantUrl;
  }

  if (options.cacheDir) {
    env.RUNE_CACHE_DIR = options.cacheDir;
  }

  if (options.noQdrant) {
    env.RUNE_ENABLE_SEMANTIC = 'false';
  }

  // Start MCP server
  console.log('Starting Rune MCP Server...');
  console.log(`Workspace: ${options.workspace}`);
  
  const server = spawn('node', [join(__dirname, '..', 'dist', 'index.js')], {
    stdio: 'inherit',
    env,
  });

  // Handle signals for graceful shutdown
  const shutdown = (signal) => {
    console.log(`\nReceived ${signal}, shutting down...`);
    server.kill(signal);
    process.exit(0);
  };

  process.on('SIGTERM', () => shutdown('SIGTERM'));
  process.on('SIGINT', () => shutdown('SIGINT'));

  // Handle server exit
  server.on('exit', (code) => {
    if (code !== 0) {
      console.error(`Server exited with code ${code}`);
      process.exit(code);
    }
  });
}

// Run main function
main().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});