#!/bin/bash
# Dev build script for local Mac ARM testing.
# Version: current version's last digit + 1, suffixed "-dev-{branch}".
set -e

cd "$(dirname "$0")/.."

# Read current version from tauri.conf.json
VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed 's/.*: *"\(.*\)",/\1/')
echo "Current version: $VERSION"

# Get current branch name
BRANCH=$(git rev-parse --abbrev-ref HEAD)
# Sanitize branch name: replace non-alphanumeric characters with hyphens
BRANCH_CLEAN=$(echo "$BRANCH" | sed 's/[^a-zA-Z0-9._-]/-/g')
echo "Current branch: $BRANCH"

# Calculate dev version: increment last digit, add -dev-{branch} suffix
BASE="${VERSION%.*}"
PATCH="${VERSION##*.}"
DEV_VERSION="${BASE}.$((PATCH + 1))-dev-${BRANCH_CLEAN}"
echo "Dev version: ${DEV_VERSION}"

# Create a temporary dev config that overrides only the version
DEV_CONFIG=$(mktemp -t skills-panel-dev-config)
cleanup() {
  rm -f "$DEV_CONFIG"
}
trap cleanup EXIT

echo "{\"version\":\"${DEV_VERSION}\"}" > "$DEV_CONFIG"

echo ""
echo "=== Building Skills CLI for aarch64-apple-darwin ==="
echo ""

cd src-tauri
# Create placeholder for Tauri externalBin check
touch skills-cli-aarch64-apple-darwin
# Build CLI
cargo build --release --bin skills-cli --features cli --target aarch64-apple-darwin
# Copy actual CLI binary with target triple suffix
cp target/aarch64-apple-darwin/release/skills-cli skills-cli-aarch64-apple-darwin
cd ..

echo ""
echo "=== Building Skills Panel v${DEV_VERSION} for aarch64-apple-darwin (debug) ==="
echo ""

npx tauri build \
  --config "$DEV_CONFIG" \
  --target aarch64-apple-darwin \
  --debug \
  --bundles dmg

echo ""
echo "Done! Built Skills Panel v${DEV_VERSION} for Mac ARM."
echo "You can find the app in: src-tauri/target/aarch64-apple-darwin/debug/"
echo ""
