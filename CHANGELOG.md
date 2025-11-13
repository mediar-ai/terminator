# Changelog

All notable changes to Terminator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.23.2] - 2025-11-12

### Fixed
- Browser scripts: Env variables now injected as single `env` object instead of separate const declarations - scripts access via `env.variableName`
- CLI: Fixed version sync to update @mediar-ai/terminator-* optionalDependencies
- Package: Updated platform package optionalDependencies from 0.22.20 to 0.23.2

## [0.23.1] - 2025-11-12

### Added
- Browser scripts: Env variable injection for file-based scripts - variables passed in `env` option are auto-injected as `const` declarations
- MCP: Cancellation support for execute_sequence workflows
- MCP: stop_execution tool for cancelling active workflows
- Extension Bridge: Proxy mode for subprocesses
- Subprocess: Inherit parent environment variables in commands

### Changed
- Dependencies: Bump terminator platform dependencies to 0.22.20
- Logging: Remove verbose logging from Windows engine and element implementation

### Fixed
- Documentation: Emphasize always using ui_diff_before_after parameter
- Line endings: Normalize line endings in example files

## [0.23.0] - 2025-11-12

### Changed
- Minor version bump

## [0.22.25] - 2025-11-12

### Fixed
- TypeScript: Use module augmentation instead of conflicting interface declarations to properly extend Desktop/Element classes

## [0.22.24] - 2025-11-12

### Fixed
- TypeScript: Explicitly re-export Desktop and other classes in wrapper.d.ts to fix "only refers to a type" errors in workflow package

## [0.22.23] - 2025-11-12

### Changed
- Code quality: Run cargo fmt to fix formatting issues

## [0.22.22] - 2025-11-12

### Fixed
- Build: Uncommented terminator-python in workspace members to fix Python wheels CI build

## [0.22.21] - 2025-11-12

### Fixed
- CI: Ensure WebSocket module is available for extension bridge test
- CI: Move WebSocket test to separate script file to fix YAML syntax
- CI: Add WebSocket bridge connection test and extension wake-up steps
- CI: Add extension loading verification step
- CI: Fix Rust formatting issues and make browser extension tests continue-on-error
- Browser: Use Browser instead of BrowserType in tests
- Browser: Use Chrome browser explicitly in browser extension tests
- Windows: Prevent Chrome-based browsers from killing all windows on close
- Desktop: Use .first() instead of .wait() for desktop.locator() API
- Rust: Fix warnings in Windows applications module
- Browser: Improve Developer mode detection in Chrome extension install workflow
- CI: Launch Chrome before running extension install workflow
- CI: Launch Chrome with extension via command line instead of UI automation
- CI: Ignore checksums for Chrome install (updates frequently)
- Clippy: Inline format args to fix warnings
- Browser: Automatically recover from debugger detachment in browser extension (#354)

### Changed
- Windows: Optimize Chrome detection to query only target process
- MCP: Remove get_focused_window_tree tool and add verification system to action tools
- MCP: Add verify_post_action helper for post-action verification

### Added
- Tests: Add test examples for parent chain, PID window, and verify window scope
- Screenshots: Add PID support and auto-resize to capture_element_screenshot
- Documentation: Update server instructions with new best practices
- Windows: Optimize Windows application lookup with EnumWindows API and caching

## [0.22.20] - 2025-11-11

### Added
- Workflow SDK: Improved TypeScript workflow type safety and error handling

### Fixed
- Windows: Prevent wrapper from exiting when restarting crashed MCP server (#350)
- MCP: Remove unnecessary VC++ redistributables check (#351)

## [0.22.16] - 2025-11-07

### Fixed
- MCP: Fixed compilation error by adding missing UINode import in helpers.rs
- MCP: Fixed TreeOutputFormat ownership issue in format_tree_string function
- MCP: Removed duplicate inputs_json serialization in workflow_typescript.rs

## [0.22.15] - 2025-11-07

### Changed
- Workflow SDK: Clean architecture refactor - eliminated ~100 lines of hardcoded JavaScript wrapper in MCP server
- Workflow SDK: `workflow.run()` now accepts optional step control parameters (`startFromStep`, `endAtStep`) and automatically skips `onError` handlers during testing
- MCP: Simplified TypeScript workflow execution by passing step control options directly to `workflow.run()`

## [0.22.13] - 2025-11-05

### Changed
- CI: Removed all macOS runners from GitHub Actions workflows to reduce costs (ci-wheels, publish-npm, publish-mcp, publish-cli)
- Documentation: Fixed typo in README and revised project description

## [0.22.12] - 2025-11-05

### Changed
- Documentation: Added TypeScript workflow context.data integration notes

## [0.22.11] - 2025-11-05

### Added
- MCP: TypeScript workflows now support context.data for passing execution results

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
