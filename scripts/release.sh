#!/bin/bash
set -euo pipefail

# Usage: ./scripts/release.sh <patch|minor|major>
# Example: ./scripts/release.sh patch   # 0.1.0 -> 0.1.1
# Example: ./scripts/release.sh minor   # 0.1.0 -> 0.2.0
# Example: ./scripts/release.sh major   # 0.1.0 -> 1.0.0

BUMP_TYPE="${1:-}"

if [[ -z "$BUMP_TYPE" ]] || [[ ! "$BUMP_TYPE" =~ ^(patch|minor|major)$ ]]; then
    echo "Usage: $0 <patch|minor|major>"
    echo ""
    echo "  patch  - Bug fixes (0.1.0 -> 0.1.1)"
    echo "  minor  - New features (0.1.0 -> 0.2.0)"
    echo "  major  - Breaking changes (0.1.0 -> 1.0.0)"
    exit 1
fi

# Ensure we're on main and clean
BRANCH=$(git branch --show-current)
if [[ "$BRANCH" != "main" ]]; then
    echo "Error: Must be on main branch (currently on $BRANCH)"
    exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
    echo "Error: Working directory is not clean. Commit or stash changes first."
    exit 1
fi

# Pull latest
git pull --rebase

# Get current version from Cargo.toml
CURRENT=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

# Calculate new version
case "$BUMP_TYPE" in
    patch) PATCH=$((PATCH + 1)) ;;
    minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
    major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
esac
NEW_VERSION="$MAJOR.$MINOR.$PATCH"

echo "Bumping version: $CURRENT -> $NEW_VERSION"
echo ""

# Update Cargo.toml
sed -i '' "s/^version = \"$CURRENT\"/version = \"$NEW_VERSION\"/" Cargo.toml

# Update Cargo.lock
cargo check > /dev/null 2>&1

# Verify
echo "Updated Cargo.toml:"
grep '^version = ' Cargo.toml | head -1

# Run checks
echo ""
echo "Running checks..."
cargo test --quiet
cargo clippy --quiet -- -D warnings
cargo fmt --check
echo "All checks passed."

# Commit, tag, and push
echo ""
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to $NEW_VERSION"
git tag "v$NEW_VERSION"
git push
git push origin "v$NEW_VERSION"

echo ""
echo "Released v$NEW_VERSION"
echo "GitHub Actions will build and publish binaries automatically."
echo "Track progress: https://github.com/pererikbergman/noupling/actions"
