#!/bin/bash

echo "================================================"
echo "Rune Standalone MCP Server Comprehensive Test Suite"
echo "================================================"

# Use the existing test_workspace
WORKSPACE_PATH="$(pwd)/test_workspace"

# Check if Qdrant is running
if ! curl -s http://127.0.0.1:6333/health >/dev/null 2>&1; then
    echo "❌ Qdrant is not running. Please start it with:"
    echo "   docker-compose -f docker-compose-qdrant.yml up -d"
    exit 1
fi

echo "✅ Qdrant is running"
echo "Using workspace: $WORKSPACE_PATH"

# Helper function for basic tests
run_test() {
    local desc=$1
    local json=$2
    echo ""
    echo "Test: $desc"
    echo "-----------------------------------"
    echo "$json" | RUST_LOG=error RUNE_ENABLE_SEMANTIC=true RUNE_WORKSPACE="$WORKSPACE_PATH" node mcp-server/dist/index.js 2>/dev/null | \
        python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if 'result' in data and 'content' in data['result']:
        result = json.loads(data['result']['content'][0]['text'])
        print(f\"✓ Found {result['total_matches']} matches in {result['search_time_ms']}ms\")
        if result['results']:
            print(f\"  First match: {result['results'][0]['file_path']} line {result['results'][0]['line_number']}\")
    elif 'error' in data:
        print(f\"✗ Error: {data['error']['message']}\")
    else:
        print('✓ Success')
except Exception as e:
    print(f'✗ Parse error: {e}')
" 2>/dev/null || echo "✗ Failed"
}

# Helper function for semantic search with detailed results
test_semantic() {
    local query=$1
    local expected=$2
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Query: \"$query\""
    echo "Expected: $expected"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    local json="{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"name\":\"search\",\"arguments\":{\"query\":\"$query\",\"mode\":\"semantic\",\"limit\":5}},\"id\":1}"
    
    echo "$json" | RUST_LOG=error RUNE_ENABLE_SEMANTIC=true RUNE_WORKSPACE="$WORKSPACE_PATH" node mcp-server/dist/index.js 2>/dev/null | \
        python3 -c "
import sys, json

try:
    data = json.load(sys.stdin)
    if 'result' in data and 'content' in data['result']:
        result = json.loads(data['result']['content'][0]['text'])
        
        print(f\"✓ Found {result['total_matches']} matches in {result['search_time_ms']}ms\")
        print(\"\\nTop results:\")
        
        for i, r in enumerate(result['results'][:5], 1):
            filename = r['file_path'].split('/')[-1]
            first_line = r['content'].split('\\\\n')[0][:80] if '\\\\n' in r['content'] else r['content'][:80]
            print(f\"  {i}. {filename}:{r['line_number']} (score: {r['score']:.4f})\")
            print(f\"     {first_line}...\")
            
        expected_file = '$expected'
        if expected_file != 'multiple':
            found_expected = any(expected_file in r['file_path'] for r in result['results'])
            if found_expected:
                print(f\"\\n✅ PASS: Found expected file '{expected_file}' in results\")
            else:
                print(f\"\\n⚠️  WARNING: Expected file '{expected_file}' not in top results\")
            
    else:
        print(f\"✗ Error: {data.get('error', {}).get('message', 'Unknown error')}\")
        
except Exception as e:
    print(f'✗ Parse error: {e}')
"
}

# Initialize MCP protocol
echo ""
echo "Initializing MCP protocol..."
echo '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"0.1.0","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":1}' | \
    RUST_LOG=error RUNE_ENABLE_SEMANTIC=true RUNE_WORKSPACE="$WORKSPACE_PATH" node mcp-server/dist/index.js 2>/dev/null | \
    python3 -c "import sys, json; d=json.load(sys.stdin); print('✓ Initialized' if 'result' in d else '✗ Failed')"

# Trigger indexing by doing a simple search
echo "Triggering workspace indexing..."
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"test","mode":"literal"}},"id":1}' | \
    RUST_LOG=error RUNE_ENABLE_SEMANTIC=true RUNE_WORKSPACE="$WORKSPACE_PATH" node mcp-server/dist/index.js 2>/dev/null > /dev/null
echo "Waiting for indexing to complete..."
sleep 5

echo ""
echo "=========================================="
echo "BASIC SEARCH MODES"
echo "=========================================="

# Test all search modes
run_test "1. Literal search: 'def'" '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"def","mode":"literal"}},"id":3}'

run_test "2. Regex search: 'struct.*{'" '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"struct.*\\{","mode":"regex"}},"id":4}'

run_test "3. Symbol search: 'Stack'" '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"Stack","mode":"symbol"}},"id":5}'

run_test "4. Hybrid search: 'function'" '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"function","mode":"hybrid","limit":5}},"id":7}'

run_test "5. File pattern: Python only" '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"def","mode":"literal","file_patterns":["*.py"]}},"id":8}'

echo ""
echo "=========================================="
echo "SEMANTIC SEARCH TESTS"
echo "=========================================="

# Semantic search tests
test_semantic "database connection pooling SQL queries" "database_operations.py"
test_semantic "password hashing user authentication security" "authentication.js"
test_semantic "HTTP REST API client retry logic" "network_client.go"
test_semantic "neural network backpropagation training" "machine_learning.py"
test_semantic "LRU cache eviction time to live" "caching_system.rs"
test_semantic "secure user login session management" "authentication.js"
test_semantic "gradient descent optimization algorithm" "machine_learning.py"
test_semantic "websocket real-time communication protocol" "network_client.go"
test_semantic "save data to disk persistently" "database_operations.py"
test_semantic "text processing string manipulation" "string_utils.js"
test_semantic "arithmetic calculations mathematical functions" "math_operations.py"
test_semantic "error handling retry mechanism fault tolerance" "network_client.go"
test_semantic "performance optimization caching memory efficiency" "caching_system.rs"

echo ""
echo "=========================================="
echo "CROSS-FILE SEMANTIC TESTS"
echo "=========================================="

test_semantic "data storage and retrieval" "multiple"
test_semantic "security and access control" "multiple"
test_semantic "optimization techniques" "multiple"

echo ""
echo "=========================================="
echo "RAW JSON OUTPUT TEST (for debugging)"
echo "=========================================="
echo "Testing semantic search with raw JSON output..."
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"database connection pooling","mode":"semantic","limit":3}},"id":1}' | \
    RUST_LOG=error RUNE_ENABLE_SEMANTIC=true RUNE_WORKSPACE="$WORKSPACE_PATH" node mcp-server/dist/index.js 2>/dev/null | \
    python3 -m json.tool | head -100

echo ""
echo "=========================================="
echo "QDRANT STATUS"
echo "=========================================="

# Check Qdrant collections
echo "Checking Qdrant collections..."
curl -s http://127.0.0.1:6333/collections 2>/dev/null | \
    python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    collections = data.get('result', {}).get('collections', [])
    if collections:
        print(f'✓ Found {len(collections)} collection(s):')
        for col in collections:
            print(f\"  - {col['name']}\")
    else:
        print('⚠️  No collections found')
except Exception as e:
    print(f'✗ Failed to parse Qdrant response: {e}')
" || echo "✗ Qdrant check failed"

# Get collection stats if available
COLLECTION_NAME=$(curl -s http://127.0.0.1:6333/collections 2>/dev/null | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    collections = data.get('result', {}).get('collections', [])
    if collections:
        # Find the rune collection
        for col in collections:
            if 'rune' in col['name']:
                print(col['name'])
                break
except:
    pass
" 2>/dev/null)

if [ ! -z "$COLLECTION_NAME" ]; then
    echo ""
    echo "Collection details for $COLLECTION_NAME:"
    curl -s "http://127.0.0.1:6333/collections/$COLLECTION_NAME" 2>/dev/null | \
        python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    r = d.get('result', {})
    if 'config' in r:
        print(f\"  Vector dimensions: {r['config']['params']['vectors']['size']}\")
        print(f\"  Points indexed: {r.get('points_count', 0)}\")
        print(f\"  Status: {r.get('status', 'unknown')}\")
    else:
        print('  No configuration found')
except Exception as e:
    print(f'  Failed to parse details: {e}')
" 2>/dev/null || echo "  Failed to get collection details"
fi

echo ""
echo "=========================================="
echo "PERFORMANCE SUMMARY"
echo "=========================================="

# Get stats from the engine
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"index_status","arguments":{}},"id":1}' | \
    RUST_LOG=error RUNE_ENABLE_SEMANTIC=true RUNE_WORKSPACE="$WORKSPACE_PATH" node mcp-server/dist/index.js 2>/dev/null | \
    python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if 'result' in data and 'content' in data['result']:
        stats = json.loads(data['result']['content'][0]['text'])
        print(f\"✓ Indexed files: {stats.get('indexed_files', 0)}\")
        print(f\"✓ Total symbols: {stats.get('total_symbols', 0)}\")
        print(f\"✓ Index size: {stats.get('index_size_bytes', 0):,} bytes\")
        if 'semantic_points' in stats:
            print(f\"✓ Semantic points: {stats.get('semantic_points', 0)}\")
except Exception as e:
    print(f'Could not retrieve stats: {e}')
"

echo ""
echo "================================================"
echo "✅ Test suite complete!"
echo "================================================"