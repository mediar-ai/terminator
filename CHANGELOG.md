# Changelog

All notable changes to Terminator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.22.10] - 2025-11-04

### Added
- MCP: Multi-instance mode with smart parent process checking for running multiple MCP servers
- Workflow SDK: TypeScript workflows now have full feature parity with YAML workflows (partial execution, state restoration)
- Testing: TERMINATOR_MCP_BINARY env var support for local binary testing without publishing

### Fixed
- Workflow SDK: TypeScript workflow execution now properly uses WorkflowRunner for advanced features
- Tests: MCP integration test selectors fixed to use `role:Window` to avoid matching taskbar buttons
- Workflow SDK: Made WorkflowExecutionResult fields optional to support both SDK and runner formats

## [0.22.9] - 2025-11-04

### Added
- CLI: Automatic peerDependencies update for @mediar-ai/terminator in workflow package during version sync
- Workflow format detection: Added support for `terminator.ts` as workflow entry file (alongside workflow.ts and index.ts)

### Fixed
- CI: Workflow package publish now waits for @mediar-ai/terminator to be available on NPM, preventing race condition errors
- CI: Added dependency sequencing between publish-npm and publish-workflow workflows with 10-minute timeout

### Changed
- Workflow SDK: MCP integration tests refactored to use stdio transport with npx instead of hardcoded binary paths

## [0.22.8] - 2025-11-04

### Changed
- Documentation: Updated CLAUDE.md with CLI workflow execution examples and best practices

## [0.22.7] - 2025-11-04

### Changed
- CI: Upgraded macOS runners from macos-13/14 to macos-15
- CI: Removed x86_64 macOS builds (Intel) - only ARM64 (Apple Silicon) supported going forward

## [0.22.6] - 2025-11-04

### Fixed
- `nativeid:` selector depth limit increased from 50 to 500 for deep browser web applications - fixes element finding in complex web apps like Best Plan Pro running in Chrome where UI trees can be 100+ levels deep
- Workflow SDK peer dependency updated to `^0.22.0` for better compatibility

### Changed
- Flaky browser wait test now ignored in CI to improve build reliability

## [0.22.5] - 2025-11-04

### Fixed
- Chain selector parsing with outer parentheses - selectors like `(role:Window && name:Calculator) >> (role:Custom && nativeid:NavView)` now parse correctly at runtime

### Changed
- Separated selector tests into dedicated `selector_tests.rs` file for better code organization
- Reduced `selector.rs` from 1,129 to 621 lines (implementation only)

## [0.22.2] - 2025-11-03

### Added
- Debug tests for selector functionality

### Fixed
- Cleanup of problematic selector tests

### Changed
- Updated issue creation link in skill.md

## [0.22.1] - 2025-11-03

### Fixed
- Boolean selectors handling
- UIA element capture timeout increased to 5s with automatic fallback

### Changed
- Workflow recorder timeout improvements

## [0.20.6] - 2025-10-31

### Added
- Initial CHANGELOG.md
- Release command for automated version management
