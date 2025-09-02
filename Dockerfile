# syntax=docker/dockerfile:1
ARG NODE_VERSION=22
ARG RUST_VERSION=1.89

# ============== Build Stage: Rust Components ==============
FROM rust:${RUST_VERSION}-slim-bookworm AS rust-builder

# Install build dependencies for Debian
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    build-essential \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy Rust project files
COPY Cargo.toml Cargo.lock ./
COPY rune-core ./rune-core
COPY rune-bridge ./rune-bridge

# Build with cache mounts for Cargo
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release --all-features

# ============== Build Stage: Node.js/TypeScript ==============
FROM node:${NODE_VERSION}-slim AS node-builder

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
RUN npm run build:ts && \
    npm prune --omit=dev

# ============== Build Stage: Qdrant ==============
FROM debian:trixie-slim AS qdrant-downloader

RUN apt-get update && apt-get install -y --no-install-recommends \
    wget \
    tar \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Download Qdrant binary for Linux (glibc version, not musl)
RUN wget -q https://github.com/qdrant/qdrant/releases/download/v1.12.0/qdrant-x86_64-unknown-linux-gnu.tar.gz && \
    tar -xzf qdrant-x86_64-unknown-linux-gnu.tar.gz && \
    chmod +x qdrant

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
    && echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_22.x nodistro main" > /etc/apt/sources.list.d/nodesource.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install s6-overlay for process supervision (glibc version)
ARG S6_OVERLAY_VERSION=3.2.0.0
ADD https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-noarch.tar.xz /tmp
ADD https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-x86_64.tar.xz /tmp
RUN apt-get update && apt-get install -y --no-install-recommends xz-utils && \
    tar -C / -Jxpf /tmp/s6-overlay-noarch.tar.xz && \
    tar -C / -Jxpf /tmp/s6-overlay-x86_64.tar.xz && \
    rm /tmp/*.tar.xz && \
    apt-get remove -y xz-utils && \
    apt-get autoremove -y && \
    rm -rf /var/lib/apt/lists/*

# Install runtime dependencies only
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user (Debian style)
RUN groupadd -g 1001 rune && \
    useradd -m -u 1001 -g rune -s /bin/bash rune

# Create necessary directories
RUN mkdir -p /app /data/qdrant /data/cache /workspace /config && \
    chown -R rune:rune /app /data /workspace /config

# Copy Qdrant binary
COPY --from=qdrant-downloader --chown=rune:rune /qdrant /usr/local/bin/qdrant

# Copy Rust native module - handle different possible output names
COPY --from=rust-builder --chown=rune:rune \
    /build/target/release/librune_bridge.* \
    /app/rune.node

# Copy built application and production dependencies from npm build
COPY --from=node-builder --chown=rune:rune /app/dist /app/dist
COPY --from=node-builder --chown=rune:rune /app/node_modules /app/node_modules
COPY --from=node-builder --chown=rune:rune /app/package.json /app/

# Copy s6 service definitions
COPY --chown=rune:rune docker/s6-services /etc/s6-overlay/s6-rc.d

# Copy IDE configuration templates
COPY --chown=rune:rune docker/configs /config

# Set working directory
WORKDIR /app

# Switch to non-root user
USER rune

# Environment variables
ENV NODE_ENV=production \
    QDRANT_URL=http://localhost:6334 \
    RUNE_WORKSPACE=/workspace \
    RUNE_CACHE_DIR=/data/cache \
    QDRANT_STORAGE=/data/qdrant

# Expose MCP port
EXPOSE 3333

# Health check
HEALTHCHECK --interval=30s --timeout=10s \
    --start-period=60s --retries=3 \
    CMD curl -f http://localhost:3333/health || exit 1

# Use s6-overlay as init
ENTRYPOINT ["/init"]

# OCI Labels
ARG BUILD_DATE
ARG GIT_COMMIT
ARG VERSION
LABEL org.opencontainers.image.created="${BUILD_DATE}" \
    org.opencontainers.image.source="https://github.com/rune-mcp/server" \
    org.opencontainers.image.version="${VERSION}" \
    org.opencontainers.image.revision="${GIT_COMMIT}" \
    org.opencontainers.image.vendor="Rune MCP" \
    org.opencontainers.image.licenses="MIT" \
    org.opencontainers.image.title="Rune MCP Server" \
    org.opencontainers.image.description="High-performance MCP code context engine with embedded Qdrant"
