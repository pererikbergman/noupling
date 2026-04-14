#!/bin/bash
set -euo pipefail

# Install noupling as noupling-dev alongside the stable production binary.
# Usage: ./scripts/install-dev.sh

echo "Building noupling from current branch..."
cargo build --release

# Get version + git hash for dev identification
VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
GIT_HASH=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BRANCH=$(git branch --show-current 2>/dev/null || echo "unknown")

# Copy as noupling-dev
DEST="${CARGO_HOME:-$HOME/.cargo}/bin/noupling-dev"
cp target/release/noupling "$DEST"
chmod +x "$DEST"

echo ""
echo "Installed as: noupling-dev"
echo "Version: $VERSION ($BRANCH @ $GIT_HASH)"
echo "Location: $DEST"
echo ""
echo "You now have both:"
echo "  noupling     - stable production version"
echo "  noupling-dev - development build from $BRANCH"
