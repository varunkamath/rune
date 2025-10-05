#!/bin/bash

# Test multi-word search with the Rune MCP server

echo "Starting Qdrant (if not already running)..."
docker-compose -f docker-compose-qdrant.yml up -d

echo "Setting up test environment..."
export RUNE_WORKSPACE="/Users/varun/Projects/rune"
export RUNE_CACHE_DIR=".rune_cache_test"
export RUNE_ENABLE_SEMANTIC=true
export QDRANT_URL="http://localhost:6334"
export RUST_LOG=debug

echo "Building and starting the MCP server..."
cd mcp-server

# Create a test script to send search query
cat > test_search.js << 'EOF'
import { RuneBridge } from './dist/bridge.js';

async function testSearch() {
    const bridge = new RuneBridge();

    // Initialize
    const config = {
        workspaceRoots: ["/Users/varun/Projects/rune"],
        cacheDir: ".rune_cache_test",
        enableSemantic: true
    };

    await bridge.initialize(JSON.stringify(config));
    await bridge.start();

    // Wait for indexing
    console.log("Waiting for indexing...");
    await new Promise(resolve => setTimeout(resolve, 5000));

    // Test multi-word search
    const query = {
        query: "ports grpc rest",
        mode: "literal",
        limit: 10,
        offset: 0
    };

    console.log("\n=== Testing literal search for 'ports grpc rest' ===");
    const results = await bridge.search(JSON.stringify(query));
    const parsed = JSON.parse(results);

    console.log(`Found ${parsed.results.length} results`);
    if (parsed.results.length > 0) {
        console.log("\nFirst few results:");
        parsed.results.slice(0, 3).forEach((r, i) => {
            console.log(`\n${i+1}. ${r.file_path}:${r.line_number}`);
            console.log(`   Content: ${r.content.substring(0, 100)}`);
            console.log(`   Match type: ${r.match_type}`);
            console.log(`   Score: ${r.score}`);
        });
    }

    await bridge.stop();
}

testSearch().catch(console.error);
EOF

node test_search.js

# Cleanup
rm test_search.js
