#!/bin/bash

echo "=========================================="
echo "Testing File Change Detection and Indexing"
echo "=========================================="

# Create a test workspace directory
TEST_DIR="test_workspace_auto_reindex"
rm -rf $TEST_DIR
mkdir -p $TEST_DIR

# Create an initial test file
echo "function testFunction() { return 42; }" > $TEST_DIR/initial.js

# Function to call MCP server with proper initialization
call_mcp_with_init() {
    local search_json="$1"

    # Create a temporary file for the JSON-RPC sequence
    local temp_file=$(mktemp)

    # Write initialization and search request
    cat > "$temp_file" << EOF
{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"0.1.0","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":0}
$search_json
EOF

    # Send both requests and extract the search result
    cat "$temp_file" | RUNE_WORKSPACE="$TEST_DIR" node mcp-server/dist/index.js 2>/dev/null | tail -n 1

    rm -f "$temp_file"
}

echo ""
echo "Test 1: Initial indexing..."
result=$(call_mcp_with_init '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"testFunction","mode":"literal"}},"id":1}')
echo "$result" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if 'result' in data:
        result = json.loads(data['result']['content'][0]['text'])
        if result['total_matches'] > 0:
            print(f'✓ Found {result[\"total_matches\"]} matches for testFunction')
        else:
            print('✗ testFunction not found in initial index')
except Exception as e:
    print(f'✗ Failed to parse response: {e}')
"

# Test 2: Add a new file
echo ""
echo "Test 2: Adding new file 'newfile.js'..."
echo "function newFunction() { return 'new'; }" > $TEST_DIR/newfile.js

# The next call should index the new file
result=$(call_mcp_with_init '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"newFunction","mode":"literal"}},"id":1}')
echo "$result" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if 'result' in data:
        result = json.loads(data['result']['content'][0]['text'])
        if result['total_matches'] > 0:
            print(f'✓ New file indexed: Found {result[\"total_matches\"]} matches')
            if result['results']:
                print(f'  File: {result[\"results\"][0][\"file_path\"]}')
        else:
            print('✗ New file not indexed')
except Exception as e:
    print(f'✗ Failed to parse response: {e}')
"

# Test 3: Modify existing file
echo ""
echo "Test 3: Modifying 'initial.js'..."
echo "function testFunction() { return 42; }
function modifiedFunction() { return 'modified'; }" > $TEST_DIR/initial.js

result=$(call_mcp_with_init '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"modifiedFunction","mode":"literal"}},"id":1}')
echo "$result" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if 'result' in data:
        result = json.loads(data['result']['content'][0]['text'])
        if result['total_matches'] > 0:
            print(f'✓ Modified content indexed: Found {result[\"total_matches\"]} matches')
        else:
            print('✗ Modified content not indexed')
except Exception as e:
    print(f'✗ Failed to parse response: {e}')
"

# Test 4: Delete a file and verify it's removed from index
echo ""
echo "Test 4: Deleting 'newfile.js'..."
rm $TEST_DIR/newfile.js

result=$(call_mcp_with_init '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search","arguments":{"query":"newFunction","mode":"literal"}},"id":1}')
echo "$result" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if 'result' in data:
        result = json.loads(data['result']['content'][0]['text'])
        if result['total_matches'] == 0:
            print('✓ Deleted file removed from index')
        else:
            print(f'✗ Deleted file still in index: Found {result[\"total_matches\"]} matches')
            if result['results']:
                for r in result['results']:
                    print(f'  Still found in: {r[\"file_path\"]}')
except Exception as e:
    print(f'✗ Failed to parse response: {e}')
"

# Test 5: Check final index status
echo ""
echo "Test 5: Final index status..."
result=$(call_mcp_with_init '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"index_status","arguments":{}},"id":1}')
echo "$result" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if 'result' in data:
        result = json.loads(data['result']['content'][0]['text'])
        print(f'✓ Indexed files: {result.get(\"indexed_files\", 0)}')
        print(f'  Total symbols: {result.get(\"total_symbols\", 0)}')
        print(f'  Index size: {result.get(\"index_size_bytes\", 0)} bytes')
        # Should be 1 file (initial.js) after deleting newfile.js
        expected_files = 1
        actual_files = result.get(\"indexed_files\", 0)
        if actual_files == expected_files:
            print(f'  ✓ File count correct: {actual_files}')
        else:
            print(f'  ✗ File count mismatch: expected {expected_files}, got {actual_files}')
except Exception as e:
    print(f'✗ Failed to parse response: {e}')
"

# Clean up
echo ""
echo "Cleaning up..."
rm -rf $TEST_DIR

echo ""
echo "=========================================="
echo "File Change Detection Test Complete!"
echo "=========================================="
echo ""
echo "Note: This test verifies that file changes are detected"
echo "and indexed correctly when the MCP server starts fresh."
echo "The delete_file_metadata fix ensures deleted files are"
echo "properly removed from the index."
