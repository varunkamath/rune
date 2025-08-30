#!/usr/bin/env node

/**
 * Postinstall script for @rune/mcp-server
 * This script handles platform-specific binary distribution
 * for npm packages in the future.
 */

const { existsSync, copyFileSync } = require('fs');
const { join, dirname } = require('path');
const { platform, arch } = require('os');

const BINARY_NAME = 'rune.node';
const ROOT_DIR = join(__dirname, '..');

// Map Node.js platform/arch to Rust target triples
const PLATFORM_MAP = {
  'darwin-x64': 'darwin-x64',
  'darwin-arm64': 'darwin-arm64',
  'linux-x64': 'linux-x64-gnu',
  'linux-arm64': 'linux-arm64-gnu',
  'win32-x64': 'win32-x64-msvc',
  'win32-arm64': 'win32-arm64-msvc',
};

function getPlatformKey() {
  return `${platform()}-${arch()}`;
}

function findPrebuiltBinary() {
  const platformKey = getPlatformKey();
  const mappedPlatform = PLATFORM_MAP[platformKey];

  if (!mappedPlatform) {
    console.warn(`Warning: No prebuilt binary for platform ${platformKey}`);
    return null;
  }

  // Try to find platform-specific binary from optionalDependencies
  const platformPackage = `@rune/mcp-server-${mappedPlatform}`;

  try {
    // Try to resolve the platform-specific package
    const platformPackagePath = require.resolve(`${platformPackage}/package.json`);
    const platformDir = dirname(platformPackagePath);
    const binaryPath = join(platformDir, BINARY_NAME);

    if (existsSync(binaryPath)) {
      return binaryPath;
    }
  } catch (e) {
    // Platform package not installed (expected for other platforms)
  }

  return null;
}

function copyBinary(source, destination) {
  try {
    copyFileSync(source, destination);
    console.log(`✅ Copied native module from ${source} to ${destination}`);
    return true;
  } catch (error) {
    console.error(`Failed to copy binary: ${error.message}`);
    return false;
  }
}

function main() {
  const targetPath = join(ROOT_DIR, BINARY_NAME);

  // Check if binary already exists (e.g., from local build)
  if (existsSync(targetPath)) {
    console.log(`✅ Native module already exists at ${targetPath}`);
    return;
  }

  // Try to find and copy prebuilt binary
  const prebuiltPath = findPrebuiltBinary();

  if (prebuiltPath) {
    if (copyBinary(prebuiltPath, targetPath)) {
      return;
    }
  }

  // If we reach here, no binary was found
  console.log('');
  console.log('═══════════════════════════════════════════════════════════');
  console.log('⚠️  No prebuilt binary found for your platform');
  console.log('');
  console.log('To build from source:');
  console.log('  1. Install Rust: https://rustup.rs/');
  console.log('  2. Run: npm run build:bridge');
  console.log('');
  console.log(`Platform: ${getPlatformKey()}`);
  console.log('═══════════════════════════════════════════════════════════');
  console.log('');

  // Don't fail the install - the mock bridge will be used
  console.log(
    'The MCP server will run with a mock implementation until the native module is built.'
  );
}

// Only run if executed directly (not imported)
if (require.main === module) {
  main();
}
