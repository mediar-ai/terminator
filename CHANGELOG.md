# Changelog

All notable changes to Terminator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.23.21] - 2025-11-25

### Added
- MCP: Add OCR support and click_ocr_index tool for text-based UI automation
- MCP: Add OCR tree formatting with indexed words in get_window_tree
- UI Automation: Expose OCR and coordinate click on Desktop
- UI Automation: Implement Windows OCR with bounding boxes using Windows Media OCR
- Workflow Recorder: Add process_name field to event structs and TextInputTracker

### Fixed
- Telemetry: Add tracing-opentelemetry layer for TraceId propagation to logs

### Changed
- MCP: Update get_window_tree description with OCR usage information

## [0.23.20] - 2025-11-25

### Added
- MCP: Add tracing span with trace_id for distributed tracing
- Extension: Add lifecycle logging and health reporting
- UI Automation: Add element verification to open_application and navigate_browser

## [0.23.19] - 2025-11-22

### Fixed
- Telemetry: Add structured log attributes for OpenTelemetry correlation
- Build: Revert bun install mode restriction for S3 mount compatibility

## [0.23.18] - 2025-11-21

### Added
- MCP: Add version field to /health endpoint for monitoring

### Fixed
- Code quality: Address cargo clippy warnings (format strings, collapsible if, ptr_arg)

## [0.23.17] - 2025-11-21

### Added
- Telemetry: Receive and use execution_id from executor for distributed tracing

### Fixed
- Tests: Add missing trace_id and execution_id fields to test fixtures

### Changed
- Docs: Update README terminology from 'legacy systems' to 'legacy software'

## [0.23.16] - 2025-11-21

### Added
- Telemetry: Add log_source and trace_id fields for distributed tracing support

## [0.23.15] - 2025-11-21

### Added
- Workflow: Add onSuccess handler that returns data for MCP integration

### Changed
- Docs: Improve CLI installation instructions with npm wrapper guidance

## [0.23.14] - 2025-11-21

### Changed
- Workflow: Rename createWorkflowError to WorkflowError for consistency

## [0.23.13] - 2025-11-21

### Added
- CI/CD: Add GitHub Actions workflow to publish @mediar-ai/kv package
  - Automatically builds TypeScript and publishes to npm on version tags
  - Synced @mediar-ai/kv version to 0.23.13 (was stuck at 0.1.0 since v0.23.8)

## [0.23.12] - 2025-11-21

### Fixed
- CI/CD: Prevent duplicate artifact uploads in Release Terminator CLI workflow
  - Fixed "Not Found" errors by uploading only archives (*.tar.gz, *.zip) instead of all artifacts
  - Removed duplicate terminator.exe uploads that were causing workflow failures
- CI/CD: Remove duplicate tag trigger from Publish Workflow Package workflow
  - Fixed double workflow runs by removing redundant push:tags trigger
  - Now only triggers via workflow_run dependency chain after NPM packages are published
  - Prevents race conditions and ensures correct dependency order

## [0.23.11] - 2025-11-21

### Added
- Telemetry: Add OpenTelemetry metadata for better filtering and grouping in ClickHouse dashboards
  - Resource-level: deployment.environment, service.instance.id, os.type, os.arch, automation.api
  - Workflow-level: workflow.execution_id, workflow.url, workflow.format, workflow.trigger_source
  - Step-level: step.process, step.selector, step.url, step.text_length for improved filtering
- MCP: Add post-action verification to missing action tools
- MCP: Add activate_window before actions and enhance press_key_global
- MCP: Make ui_diff_before_after and include_tree_after_action mandatory
- MCP: Make verify_timeout_ms optional with 2000ms default
- MCP: Add bring_to_front flag to separate foreground from window management

### Fixed
- MCP: Use text: selector instead of value: for set_value auto-verification
- MCP: Hide #ID selectors in compact YAML tree view
- MCP: Add unwrap_or(2000) to verify_timeout_ms in verification code
- MCP: Set in_sequence flag in dispatch_tool to prevent double window management
- Windows: Use AttachThreadInput to bypass Windows focus-stealing prevention

### Changed
- MCP: Skip tree building when both verify fields are empty
- MCP: Make highlight_before_action a required boolean parameter
- MCP: Flatten FontStyle into HighlightElementArgs
- MCP: Change maximize_target default from true to false
- Editor: Add .editorconfig and fix .gitattributes line endings

### Documentation
- MCP: Improve MCP agent prompt with selector syntax guide
- MCP: Update MCP agent prompt with tool behavior defaults

## [0.23.10] - 2025-11-20

### Added
- MCP: Add local-copy execution mode for TypeScript workflows to fix S3/rclone symlink issues
- Window management: Add BringWindowToTop and SetForegroundWindow to window management

### Changed
- Workflow: Copy workflow files to local temp directory before execution for better performance and symlink support
- MCP: Default MCP_EXECUTION_MODE to "local-copy" in agent wrapper

## [0.23.9] - 2025-11-20

### Added
- MCP: Add optional window management parameters to all MCP tools
- Docs: Add Bounty Developer Program section to README

### Fixed
- Tests: Add missing skip_preflight_check and window_mgmt fields to ExecuteSequenceArgs test instantiations
- Style: Run cargo fmt to fix formatting issues

## [0.23.8] - 2025-11-20

### Added
- KV package: New @mediar-ai/kv package for workflow state sharing with Memory, File, and Redis adapters

### Changed
- CLI: Add sync_kv_package() to version management for release automation
- CI: Remove Linux from Python wheels workflow
- Test: Ignore debugger detach tests failing in CI

## [0.23.7] - 2025-11-18

### Fixed
- Linux: Add missing trait method implementations (maximize_window_keyboard, minimize_window_keyboard, get_native_window_handle)
- Linux: Fix press_key signature to match trait definition
- Linux: Add Process selector case handling in selector matching
- Element: Fix unused variable warning

### Changed
- CI: Remove Linux builds from MCP and NPM publish workflows (Windows-only for now)

## [0.23.6] - 2025-11-18

### Added
- TypeScript workflow: Export WorkflowBuilder and add function overloads for type inference
- MCP: Guidance for server-side dev log tools and screenshot investigation
- Window management: Add window manager module for optimized window state management
- Window management: Integrate UWP window management support with keyboard-based maximize/restore
- Selector: Add Process selector for targeting elements by process name
- Workflow: Add workflow_id parameter for env state persistence
- Test: Add comprehensive UWP window management tests

### Changed
- **BREAKING**: Renamed `include_tree` parameter to `include_tree_after_action` across all MCP tools and YAML workflows for clearer semantics
- **BREAKING**: Enforce mandatory process scoping to eliminate desktop-wide searches - all selectors must include `process:` prefix
- **BREAKING**: Make clear_before_typing, highlight_before_action, and click_position mandatory parameters
- **BREAKING**: Make verification parameters mandatory for all action tools
- Selector: Replace PID parameter with process selector in capture_element_screenshot
- Performance: Optimize window search depth from 10 to 5
- Performance: Remove click action delays for faster automation
- Performance: Remove bounds stability checking for faster Windows element interactions
- Refactor: Remove element IDs from compact YAML tree view
- Refactor: Consolidate window management with UWP support in MCP tools
- MCP: Update tool descriptions to recommend process selector over PID

### Fixed
- Test: Fix notepad test to use correct process selector syntax
- Test: Ignore browser script tests failing in CI due to extension connection timeout
- Selector: Restore boolean operators for non-text prefixed selectors
- Selector: Handle special characters in prefixed selectors
- Security: Update js-yaml to fix prototype pollution vulnerability
- MCP: Fix timeout inconsistency in wait_for_element error details
- MCP: Ensure window restoration in all error paths
- MCP: Add tree data management guidance to prevent redundant get_window_tree calls
- Inline autocomplete: Dismiss before pressing Enter/Return
- Workflow: Gracefully handle user cancellation in workflow execution
- CI: Resolve module resolution error in Node.js tests (#367)
- Formatting: Apply cargo fmt and fix all clippy warnings

## [0.23.5] - 2025-11-13

### Changed
- Maintenance release with dependency updates

## [0.23.4] - 2025-11-13

### Fixed
- CI: Fixed bun install failing on optional dependencies (macOS packages on Windows) by adding `|| true` to continue on error
- CI: Added ws module to devDependencies for extension bridge WebSocket test

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
