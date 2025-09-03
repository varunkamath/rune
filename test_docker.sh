#!/bin/bash

echo "================================================"
echo "Rune Docker Container Comprehensive Test Suite"
echo "================================================"

WORKSPACE_PATH="$(pwd)/test_workspace"

echo "Starting container..."
CONTAINER_ID=$(docker run -d \
    -v "$WORKSPACE_PATH:/workspace:ro" \
    -e RUNE_WORKSPACE=/workspace \
    -e RUNE_ENABLE_SEMANTIC=true \
    rune-test)

echo "Container ID: ${CONTAINER_ID:0:12}"
echo "Waiting for initialization (20 seconds)..."
sleep 20

# Helper function for basic tests
run_test() {
    local desc=$1
    local json=$2
    echo ""
    echo "Test: $desc"
    echo "-----------------------------------"
    echo "$json" | docker exec -i $CONTAINER_ID node /app/dist/index.js 2>/dev/null | \
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

    echo "$json" | docker exec -i $CONTAINER_ID node /app/dist/index.js 2>/dev/null | \
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
            first_line = r['content'].split('\\\\n')[0][:80]
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

# Initialize
echo "Initializing MCP protocol..."
echo '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"0.1.0","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":1}' | \
    docker exec -i $CONTAINER_ID node /app/dist/index.js 2>/dev/null | python3 -c "import sys, json; d=json.load(sys.stdin); print('✓ Initialized' if 'result' in d else '✗ Failed')"

# Trigger indexing
echo "Triggering workspace indexing..."
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"test","mode":"literal"}},"id":1}' | \
    docker exec -i $CONTAINER_ID node /app/dist/index.js 2>/dev/null > /dev/null
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

# First, let's see the RAW JSON output for one semantic search
echo ""
echo "RAW JSON OUTPUT TEST"
echo "===================="
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"database connection pooling","mode":"semantic","limit":5}},"id":1}' | \
    docker exec -i $CONTAINER_ID node /app/dist/index.js 2>/dev/null | \
    python3 -m json.tool | head -200

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
echo "QDRANT STATUS"
echo "=========================================="

# Check Qdrant
docker exec $CONTAINER_ID curl -s http://localhost:6333/collections | python3 -m json.tool 2>/dev/null | grep -A2 "collections" || echo "✗ Qdrant check failed"

# Get collection stats
COLLECTION_NAME=$(docker exec $CONTAINER_ID curl -s http://localhost:6333/collections | python3 -c "import sys, json; print(json.load(sys.stdin)['result']['collections'][0]['name'])" 2>/dev/null)

if [ ! -z "$COLLECTION_NAME" ]; then
    docker exec $CONTAINER_ID curl -s "http://localhost:6333/collections/$COLLECTION_NAME" | \
        python3 -c "
import sys, json
d = json.load(sys.stdin)
r = d['result']
print(f\"Collection: {r['config']['params']['vectors']['size']}-dim vectors\")
print(f\"Points indexed: {r['points_count']}\")
print(f\"Status: {r['status']}\")
" 2>/dev/null
fi

# Cleanup
echo ""
echo "Cleaning up..."
docker stop $CONTAINER_ID > /dev/null
docker rm $CONTAINER_ID > /dev/null
echo "✅ Test complete!"
