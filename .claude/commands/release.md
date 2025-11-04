---
description: Bump version and create a new release
---

Bump the version number across all packages using the terminator CLI and create a new release.

Usage:
- `/release` - Automatically bump patch version and push (one-shot, no questions)
- `/release patch` - Bump patch version (e.g., 0.20.6 → 0.20.7)
- `/release minor` - Bump minor version (e.g., 0.20.6 → 0.21.0)
- `/release major` - Bump major version (e.g., 0.20.6 → 1.0.0)

Steps:
1. Read current version from `terminator-cli/Cargo.toml`
2. If no argument provided, default to patch bump
3. Run `terminator release <patch|minor|major>` to:
   - Bump version across all packages (Cargo.toml, package.json)
   - Create git tag (v{version})
   - Push commit and tag
4. Read git commits since last version tag
5. Update CHANGELOG.md with new version section and changes
6. Commit changelog update: "chore: update CHANGELOG for v{version}"
7. Push changelog commit

Important:
- NO user confirmation needed - just do it automatically
- Uses terminator CLI for version management (syncs all packages)
- Git tag triggers CI/CD workflows:
  - `publish-npm.yml` → @mediar-ai/terminator to npm
  - `publish-mcp.yml` → terminator-mcp-agent to npm
  - `ci-wheels.yml` → Python wheels to PyPI
- Be fast and efficient - this is a one-shot command
- CHANGELOG.md follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format
