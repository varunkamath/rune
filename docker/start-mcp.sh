#!/bin/bash
# Start script for MCP server with embedded Qdrant

# Start Qdrant in the background (it will be ready when needed)
echo "Starting Qdrant in background..." >&2
/usr/local/bin/qdrant --config-path /etc/qdrant/config.yaml > /tmp/qdrant.log 2>&1 &

# Check Qdrant health in the background (non-blocking)
(
    for i in $(seq 1 60); do
        if curl -sf http://localhost:6333/ > /dev/null 2>&1; then
            echo "Qdrant is ready!" >&2
            break
        fi
        sleep 1
    done
) &

# Start the MCP server immediately (don't wait for Qdrant)
# The Rust code will handle connection retries for semantic search
exec node /app/dist/index.js
