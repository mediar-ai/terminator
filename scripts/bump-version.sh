#!/bin/bash

# Script to bump version across all workspace members
# Usage: ./scripts/bump-version.sh <new-version>

set -e

if [ $# -eq 0 ]; then
    echo "Usage: $0 <new-version>"
    echo "Example: $0 0.6.0"
    exit 1
fi

NEW_VERSION=$1
echo "Bumping version to $NEW_VERSION"

# Update workspace version in root Cargo.toml
sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml

# Update npm package version
cd bindings/nodejs
npm version $NEW_VERSION --no-git-tag-version
cd ../..

# Update Cargo.lock
cargo update -w

echo "Version bumped to $NEW_VERSION"
echo ""
echo "Next steps:"
echo "1. Review and commit the changes"
echo "2. Create a PR with these changes"
echo "3. After merging, create and push a tag:"
echo "   git tag -a v$NEW_VERSION -m \"Release v$NEW_VERSION\""
echo "   git push origin v$NEW_VERSION"