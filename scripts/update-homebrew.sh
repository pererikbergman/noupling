#!/bin/bash
set -euo pipefail

# Usage: ./scripts/update-homebrew.sh <version>
# Example: ./scripts/update-homebrew.sh 0.1.0
#
# Downloads the source tarball, calculates SHA256, and updates
# the Homebrew formula in the homebrew-noupling tap repo.

VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.1.0"
    exit 1
fi

TAG="v${VERSION}"
TARBALL_URL="https://github.com/pererikbergman/noupling/archive/refs/tags/${TAG}.tar.gz"
TAP_REPO="pererikbergman/homebrew-noupling"
FORMULA_PATH="Formula/noupling.rb"

echo "Updating Homebrew formula for ${TAG}..."

# Download tarball and calculate SHA256
echo "Downloading ${TARBALL_URL}..."
SHA256=$(curl -sL "${TARBALL_URL}" | shasum -a 256 | awk '{print $1}')

if [[ -z "$SHA256" || "$SHA256" == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" ]]; then
    echo "Error: Failed to download tarball or tag ${TAG} does not exist."
    exit 1
fi

echo "SHA256: ${SHA256}"

# Clone the tap repo
TMPDIR=$(mktemp -d)
cd "${TMPDIR}"
gh repo clone "${TAP_REPO}" tap
cd tap

# Update the formula
sed -i '' "s|url \".*\"|url \"${TARBALL_URL}\"|" "${FORMULA_PATH}"
sed -i '' "s|sha256 \".*\"|sha256 \"${SHA256}\"|" "${FORMULA_PATH}"

echo ""
echo "Updated formula:"
cat "${FORMULA_PATH}"

# Commit and push
git add "${FORMULA_PATH}"
git commit -m "Update noupling to ${VERSION}"
git push

echo ""
echo "Homebrew formula updated to ${VERSION}"
echo "Users can now run: brew upgrade noupling"

# Cleanup
rm -rf "${TMPDIR}"
