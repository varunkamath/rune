# syntax=docker/dockerfile:1
ARG NODE_VERSION=24
ARG RUST_VERSION=1.89

# ============== Chef Stage: Install cargo-chef ==============
FROM rust:${RUST_VERSION}-slim-trixie AS chef
RUN cargo install cargo-chef --locked
WORKDIR /build

# ============== Planner Stage: Generate Recipe ==============
FROM chef AS planner
# Copy all project files (cargo chef will extract what it needs)
COPY . .
# Generate recipe for dependency caching
RUN cargo chef prepare --recipe-path recipe.json

# ============== Build Stage: Rust Components ==============
FROM chef AS rust-builder

# Install build dependencies for Debian
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    build-essential \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy recipe and build dependencies (cached layer!)
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --all-features --recipe-path recipe.json

# Copy Rust project files
COPY Cargo.toml Cargo.lock ./
COPY rune-core ./rune-core
COPY rune-bridge ./rune-bridge

# Build the actual application (only reruns when source changes)
RUN cargo build --release --all-features

# ============== Build Stage: Node.js/TypeScript ==============
FROM node:${NODE_VERSION}-trixie-slim AS node-builder

WORKDIR /app

# Copy only the mcp-server package files (simplified for npm)
COPY mcp-server/package.json ./
COPY mcp-server/scripts ./scripts

# Generate package-lock.json and install all dependencies
RUN npm install

# Copy TypeScript source
COPY mcp-server/src ./src
COPY mcp-server/tsconfig.json ./

# Build TypeScript and remove dev dependencies in one layer
RUN npm run build:ts \
    && npm prune --omit=dev

# ============== Build Stage: Qdrant ==============
# Use official Qdrant Docker image which supports both amd64 and arm64
FROM qdrant/qdrant:v1.15.4 AS qdrant-source

# The official image already has the correct binary for the platform
# We just extract it for use in our production image

# ============== Production Stage ==============
FROM debian:trixie-slim AS production

# Set shell for pipefail option
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Install Node.js 22 from NodeSource repository
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    gnupg \
    && mkdir -p /etc/apt/keyrings \
    && curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg \
    && echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_22.x nodistro main" >/etc/apt/sources.list.d/nodesource.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install s6-overlay for process supervision (glibc version)
ARG S6_OVERLAY_VERSION=3.2.1.0
ADD https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-noarch.tar.xz /tmp
RUN apt-get update && apt-get install -y --no-install-recommends xz-utils curl && \
    tar -C / -Jxpf /tmp/s6-overlay-noarch.tar.xz && \
    # Download architecture-specific overlay
    ARCH=$(uname -m) && \
    if [ "$ARCH" = "aarch64" ]; then \
    curl -sSL -o /tmp/s6-overlay-arch.tar.xz https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-aarch64.tar.xz; \
    else \
    curl -sSL -o /tmp/s6-overlay-arch.tar.xz https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-x86_64.tar.xz; \
    fi && \
    tar -C / -Jxpf /tmp/s6-overlay-arch.tar.xz && \
    rm /tmp/*.tar.xz && \
    apt-get remove -y xz-utils curl && \
    apt-get autoremove -y && \
    rm -rf /var/lib/apt/lists/*

# Install runtime dependencies and debugging tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    tzdata \
    netcat-openbsd \
    procps \
    libunwind8 \
    libunwind-dev \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user (Debian style)
RUN groupadd -g 1001 rune \
    && useradd -m -u 1001 -g rune -s /bin/bash rune

# Create necessary directories
RUN mkdir -p /app /data/qdrant /data/cache /workspace /config \
    && chown -R rune:rune /app /data /workspace /config

# Copy Qdrant binary from official image
COPY --from=qdrant-source --chown=rune:rune /qdrant/qdrant /usr/local/bin/qdrant

# Copy Rust native module - find the actual .so file
RUN mkdir -p /app
COPY --from=rust-builder --chown=rune:rune /build/target/release/librune_bridge.* /app/rune.node

# Copy built application and production dependencies from npm build
COPY --from=node-builder --chown=rune:rune /app/dist /app/dist
COPY --from=node-builder --chown=rune:rune /app/node_modules /app/node_modules
COPY --from=node-builder --chown=rune:rune /app/package.json /app/

# Copy s6 service definitions and register them
COPY --chown=rune:rune docker/s6-services /etc/s6-overlay/s6-rc.d
RUN chmod +x /etc/s6-overlay/s6-rc.d/*/run \
    # Register services with s6-rc
    && touch /etc/s6-overlay/s6-rc.d/user/contents.d/qdrant \
    && touch /etc/s6-overlay/s6-rc.d/user/contents.d/rune

# Copy IDE configuration templates
COPY --chown=rune:rune docker/configs /config

# Copy start script for MCP mode
COPY --chmod=755 docker/start-mcp.sh /usr/local/bin/start-mcp

# Set working directory
WORKDIR /app

# Switch to non-root user
USER rune

# Environment variables
ENV NODE_ENV=production \
    QDRANT_URL=http://localhost:6334 \
    QDRANT__SERVICE__HOST=0.0.0.0 \
    RUNE_WORKSPACE=/workspace \
    RUNE_CACHE_DIR=/data/cache \
    RUNE_SHARED_CACHE=true \
    QDRANT_STORAGE=/data/qdrant

# Expose MCP port
EXPOSE 3333

# Health check - check if MCP server responds to JSON-RPC
# HEALTHCHECK --interval=30s --timeout=10s \
#     --start-period=90s --retries=3 \
#     CMD echo '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"0.1.0","capabilities":{}},"id":1}' | nc -w 2 localhost 3333 | grep -q "serverInfo" || exit 1

# Use s6-overlay as init
ENTRYPOINT ["/init"]

# OCI Labels
ARG BUILD_DATE
ARG GIT_COMMIT
ARG VERSION
LABEL org.opencontainers.image.created="${BUILD_DATE}" \
    org.opencontainers.image.source="https://github.com/varunkamath/rune" \
    org.opencontainers.image.version="${VERSION}" \
    org.opencontainers.image.revision="${GIT_COMMIT}" \
    org.opencontainers.image.licenses="MIT" \
    org.opencontainers.image.title="Rune MCP Server" \
    org.opencontainers.image.description="High-performance MCP code context engine with embedded Qdrant"