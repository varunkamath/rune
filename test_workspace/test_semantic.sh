#!/bin/bash
echo "Starting semantic search test with deduplication fix..."

# First trigger indexing
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"reindex","arguments":{}},"id":1}' | \
RUNE_WORKSPACE="$(pwd)" RUNE_ENABLE_SEMANTIC=true QDRANT_URL=http://localhost:6334 node ../mcp-server/dist/index.js 2>/dev/null | \
jq -r '.result.content[0].text' 2>/dev/null || echo "Reindex response parsing failed"

sleep 2

# Now test semantic search
echo "Testing semantic search for 'database connection'..."
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"database connection","mode":"semantic","limit":10}},"id":2}' | \
RUNE_WORKSPACE="$(pwd)" RUNE_ENABLE_SEMANTIC=true QDRANT_URL=http://localhost:6334 node ../mcp-server/dist/index.js 2>/dev/null > semantic_result.json

# Check for duplicates
echo "Checking for duplicate results..."
cat semantic_result.json | jq -r '.result.content[0].text' | jq -r '.results[] | "\(.file_path):\(.start_line)"' | sort | uniq -c | sort -rn | head -10

echo ""
echo "Total unique results:"
cat semantic_result.json | jq -r '.result.content[0].text' | jq -r '.results[] | "\(.file_path):\(.start_line)"' | sort -u | wc -l

echo ""
echo "Total results returned:"
cat semantic_result.json | jq -r '.result.content[0].text' | jq -r '.results[] | "\(.file_path):\(.start_line)"' | wc -l
