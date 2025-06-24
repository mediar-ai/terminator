# Release Process

This document describes the release process for the Terminator project, which includes publishing to both crates.io and npm.

## Prerequisites

Before releasing, ensure you have:

1. **Permissions**: 
   - Push access to the repository
   - Publishing rights on crates.io for `terminator` and `terminator-workflow-recorder`
   - Publishing rights on npm for `terminator.js`

2. **Secrets configured** (repository maintainers only):
   - `CRATES_IO_TOKEN`: API token for crates.io
   - `NPM_TOKEN`: API token for npm registry

## Release Steps

### 1. Prepare the Release

1. **Update version** across the workspace:
   ```bash
   ./scripts/bump-version.sh 0.6.0  # Replace with your version
   ```

2. **Update CHANGELOG.md** with release notes

3. **Run tests** to ensure everything works:
   ```bash
   cargo test --all
   cargo fmt --all -- --check
   cargo clippy --all -- -D warnings
   ```

4. **Create a PR** with these changes and get it reviewed

### 2. Create Release

After the PR is merged:

1. **Pull latest main**:
   ```bash
   git checkout main
   git pull origin main
   ```

2. **Create and push tag**:
   ```bash
   git tag -a v0.6.0 -m "Release v0.6.0"
   git push origin v0.6.0
   ```

### 3. Monitor Release

The release workflow will automatically:

1. **Publish to crates.io**:
   - `terminator` crate
   - `terminator-workflow-recorder` crate

2. **Build native binaries** for all platforms:
   - Windows (x64, ARM64)
   - macOS (x64, ARM64)
   - Linux (x64)

3. **Publish to npm**:
   - Platform-specific packages
   - Main `terminator.js` package

4. **Create GitHub release** with auto-generated notes

### 4. Verify Release

After the workflow completes:

1. Check [crates.io](https://crates.io):
   - https://crates.io/crates/terminator
   - https://crates.io/crates/terminator-workflow-recorder

2. Check [npm](https://www.npmjs.com):
   - https://www.npmjs.com/package/terminator.js

3. Check GitHub releases page

## Troubleshooting

### Failed crates.io publish

If publishing to crates.io fails:
- Check that all dependencies are published
- Ensure metadata in `Cargo.toml` is complete
- Verify the API token is valid

### Failed npm publish

If npm publishing fails:
- Check that version doesn't already exist
- Ensure all platform binaries were built successfully
- Verify the npm token is valid

### Version mismatch

The workflow checks that the git tag matches the workspace version. If they don't match:
1. Update the version in `Cargo.toml` to match the tag
2. Or delete the tag and create a new one matching the version

## Manual Publishing (Emergency Only)

If automated publishing fails, you can publish manually:

### Crates.io
```bash
cd terminator
cargo publish

cd ../terminator-workflow-recorder
cargo publish
```

### npm
```bash
cd bindings/nodejs
npm run sync-version
npm publish
```