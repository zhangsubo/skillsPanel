#!/bin/bash
# Dev build script for local Mac ARM testing.
# Version: current version's last digit + 1, suffixed "-dev".
set -e

cd "$(dirname "$0")/.."

# Read current version from tauri.conf.json
VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed 's/.*: *"\(.*\)",/\1/')
echo "Current version: $VERSION"

# Calculate dev version: increment last digit, add -dev suffix
BASE="${VERSION%.*}"
PATCH="${VERSION##*.}"
DEV_VERSION="${BASE}.$((PATCH + 1))-dev"
echo "Dev version: ${DEV_VERSION}"

# Create a temporary dev config that overrides only the version
DEV_CONFIG=$(mktemp -t skills-panel-dev-config)
cleanup() {
  rm -f "$DEV_CONFIG"
}
trap cleanup EXIT

echo "{\"version\":\"${DEV_VERSION}\"}" > "$DEV_CONFIG"

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