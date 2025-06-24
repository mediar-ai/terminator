# CI/CD Setup for Terminator

This document explains the continuous integration and deployment setup for the Terminator project.

## Overview

The project publishes to multiple package registries:
- **crates.io**: Rust crates (`terminator`, `terminator-workflow-recorder`)
- **npm**: Node.js bindings (`terminator.js`)

## GitHub Actions Workflows

### 1. `.github/workflows/release.yml` (Main Release Workflow)
This is the primary workflow that handles the complete release process:
- Triggers on version tags (`v*`)
- Publishes Rust crates to crates.io
- Builds native binaries for all platforms
- Publishes npm packages
- Creates GitHub releases

### 2. `.github/workflows/publish-crates.yml` (Standalone Crates Publishing)
- Publishes only the Rust crates to crates.io
- Can be used independently if needed

### 3. `.github/workflows/publish-npm.yml` (Deprecated)
- Original npm-only publish workflow
- Kept for reference but replaced by `release.yml`

### 4. `.github/workflows/ci.yml`
- Runs tests and checks on pull requests
- Ensures code quality before merging

## Required Secrets

Repository maintainers need to configure these secrets:

1. **`CRATES_IO_TOKEN`**
   - Get from: https://crates.io/settings/tokens
   - Permissions: publish-update for `terminator` and `terminator-workflow-recorder`

2. **`NPM_TOKEN`**
   - Get from: https://www.npmjs.com/settings/YOUR_USERNAME/tokens
   - Type: Automation token
   - Permissions: publish for `terminator.js`

## Version Management

All versions are centralized in the workspace root `Cargo.toml`:
```toml
[workspace.package]
version = "0.5.12"
```

The `scripts/bump-version.sh` script updates versions across:
- Workspace `Cargo.toml`
- `bindings/nodejs/package.json`
- `Cargo.lock`

## Release Process

1. **Bump version**:
   ```bash
   ./scripts/bump-version.sh 0.6.0
   ```

2. **Create PR** with version changes

3. **After merge, tag and push**:
   ```bash
   git tag -a v0.6.0 -m "Release v0.6.0"
   git push origin v0.6.0
   ```

4. **Monitor** the release workflow in GitHub Actions

## Package Registry Links

After a successful release, packages are available at:

- **crates.io**:
  - https://crates.io/crates/terminator
  - https://crates.io/crates/terminator-workflow-recorder

- **npm**:
  - https://www.npmjs.com/package/terminator.js
  - Platform-specific packages (e.g., `terminator.js-win32-x64-msvc`)

## Troubleshooting

### Common Issues

1. **Version mismatch**: Tag version must match workspace version
2. **Auth failures**: Check that secrets are correctly configured
3. **Build failures**: Platform-specific dependencies might need updates

### Manual Publishing

If automated publishing fails:

```bash
# Crates.io
cd terminator && cargo publish
cd ../terminator-workflow-recorder && cargo publish

# npm
cd bindings/nodejs
npm run sync-version
npm publish
```

## Maintenance

- Keep dependencies up to date
- Test the release process periodically with pre-release tags
- Monitor deprecation warnings in GitHub Actions