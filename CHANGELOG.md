# Changelog

All notable changes to Terminator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.24.20] - 2026-01-13

### Added
- MCP: Add `kill_by_execution_id` to terminate child processes by execution ID
- MCP: Kill child processes when cancelling requests in `stop_execution`

### Fixed
- MCP: Make peer notifications non-blocking to prevent 503 busy errors
- MCP: Skip empty log lines to avoid spam during shutdown

### Changed
- Build: Ignore Claude Code temp files in git

## [0.24.19] - 2026-01-09

### Added
- MCP: Broadcast progress notifications to all connected MCP clients (not just the initiating one)
- MCP: Auto-cleanup dead peers on send failure

## [0.24.18] - 2026-01-09

### Added
- MCP: Forward workflow Progress, Status, and Log events as MCP notifications
- MCP: Use Windows Job Objects with KILL_ON_JOB_CLOSE to auto-terminate child processes when agent exits

### Fixed
- MCP: Extract logs from error data for TypeScript workflow failures
- Recorder: Add early exit checks to prevent UIA traversals during stop
- MCP: Extract logs from execute_sequence results in addition to run_command
- MCP: Capture pipe logs on error path for workflow failures

## [0.24.16] - 2026-01-07

### Fixed
- MCP: Pass ORG_TOKEN as env var for KV HTTP backend - workflows now use remote KV store for duplicate detection
- Core: Add CREATE_NO_WINDOW flag to prevent PowerShell console flash
- Core: Use JPEG for screenshot disk logs (~4x smaller files)


## [0.24.12] - 2025-12-23

### Fixed
- Workflow: Send 1-based step indices in emit calls
- Test: Update get_executions_dir test for mediar path

## [0.24.11] - 2025-12-22

### Added
- Desktop: Add `closeTab` method for safe browser tab closing (#446)

### Changed
- Workflow: Emit step events in normal execution path

## [0.24.10] - 2025-12-22

### Changed
- Workflow: Emit step lifecycle events automatically

### Performance
- MCP: Switch screenshots from PNG to JPEG for ~80% size reduction

## [0.24.9] - 2025-12-22

### Fixed
- Core: Detect clicks on browser chrome (tabs/address bar) and skip DOM capture
- Core: Initialize COM before UIA calls in focus save/restore

## [0.24.8] - 2025-12-19

### Changed
- MCP: Install `@mediar-ai/workflow` and `@mediar-ai/kv` packages alongside terminator


## [0.24.7] - 2025-12-19

### Added
- MCP: Add `ask_user` tool for AI to request user clarification during tool execution
- MCP: Add elicitation support for user input during tool execution (#441)
- Desktop: Add `draw_cursor` method to ScreenshotResult and `get_cursor_position` helper
- Desktop: Add `find_parent_window` helper and `get_tree_from_element` Desktop method
- Desktop: Add `window_selector` support to `capture_screenshot` snippet
- Desktop: Add title, show_overlay, browser_dom_max_elements to `get_window_tree` snippet
- Desktop: Add retry loop and click options to TypeScript snippet generation
- Desktop: Add verification code generation for action tools in TypeScript snippets
- MCP: Include `execution_log_path` and `typescript_snippet_path` in execute_sequence/execute_ts_workflow output
- HTTP: Add POST `/mode` endpoint for ask/act mode and blocked tools

### Fixed
- Desktop: Correct cursor overlay colors for BGRA format
- Desktop: Fix verification code generation to handle empty process
- Workflow: Clarify `execute_sequence` is for UI ops only
- Process: Only kill safe processes (terminator/node/bun) when port is in use, warn for unexpected processes (#438, #439)

### Changed
- Workflow: Use `executed_without_error/executed_with_error` status naming convention
- MCP: Block tools in ask mode, update status naming convention
- Desktop: Add `restoreFocus`, `uiDiffBeforeAfter` options, improve click snippet generation

## [0.24.6] - 2025-12-18

### Changed
- Version bump only (no functional changes)

## [0.24.5] - 2025-12-17

### Added
- CLI: Add `success()` early exit example to init template
- CLI: Add README.md template with non-technical workflow description

## [0.24.4] - 2025-12-17

### Added
- Workflow: Add `setState()` method to WorkflowContext for React-style state updates
- CLI: Update init template with two-step example using `next()` and `setState()`

### Fixed
- MCP: Prevent dangling bun/node processes when MCP agent stops

### Changed
- Workflow: Update ExecutionStatus enum values (`executed_without_error`, `execution_error`)
- MCP: Add X-Workflow-Dir header support

## [0.24.3] - 2025-12-17

### Added
- MCP: Add typecheck_workflow tool for TypeScript type checking

### Fixed
- Tests: Handle missing esbuild in transpiler tests

### Changed
- MCP: Improve execution logger and update README

## [0.24.2] - 2025-12-16

### Changed
- Version bump only (no functional changes)

## [0.24.1] - 2025-12-16

### Added
- MCP: Improve execution logging with step_id tracking

### Fixed
- CLI: Sync all workspace crate versions (terminator-computer-use, terminator-rs)

## [0.24.0] - 2025-12-16

### Fixed
- CLI: Add type annotations to all callbacks in init template
- Workflow: Use `unknown` instead of `any` for better type inference in callbacks

## [0.23.51] - 2025-12-16

### Fixed
- CLI: Remove double braces in init template
- MCP: Change default `tree_max_depth` from 100 to 30 with improved guidance

### Changed
- Docs: Use GIF for autoplay demo video in README

## [0.23.50] - 2025-12-16

### Changed
- Platform: Windows-only support - removed macOS and Linux platform code, dependencies, and CI workflows
- Docs: Reorganized README with improved MCP setup instructions and new demo video

### Added
- Transpiler: TypeScript support for `execute_browser_script` with context-engineered error messages

## [0.23.49] - 2025-12-16

### Changed
- CLI: Improve init template with trigger, metadata, and context.data support

## [0.23.48] - 2025-12-15

### Added
- Workflow: Add `enabled` property to trigger configs (cron, manual, webhook)

### Changed
- CI: Remove crates.io publish workflow

## [0.23.47] - 2025-12-15

### Changed
- CI: Publish all crates to crates.io in correct dependency order
- CI: Remove Linux from CLI workflow (Windows-only)
- CI: Remove Python CI/CD workflow

### Fixed
- Fix UiDiffOptions doc test (remove non-existent field)

## [0.23.46] - 2025-12-15

### Added
- Workflow: Add context.data field for workflow output data (#431)

### Fixed
- Fix: add missing APIs to run_command description (openUrl, navigateBrowser, delay, etc.)
- Fix: re-enable result.data assertions in onSuccess tests
- Fix: replace deprecated logger with console in init templates
- Fix: resolve log pipe race condition causing lost logs

## [0.23.45] - 2025-12-15

### Added
- Workflow: Add trigger/cron scheduling support for TypeScript workflows (#427)
- MCP: Add named pipe logging for TypeScript workflows (#416)
- MCP: Add gitignore-aware file search and improved dropdown error messages (#420)

### Changed
- MCP: Prompt improvements and execution logging (#419)
- Docs: Improve verify_element and select_option documentation (#422)

### Fixed
- Fix tests
- Fix: expose globals as local variables in JS wrapper script
- Fix log pipe drain before process exit (#426)
- Fix: save focus state before window activation in type_into_element (#425)
- Element and input improvements (#424)
- Focus restoration and prompt improvements (#423)
- Clippy and fmt fixes

## [0.23.43] - 2025-12-10

### Changed
- Version bump release

## [0.23.42] - 2025-12-10

### Changed
- Version bump release

## [0.23.41] - 2025-12-10

### Added
- MCP: Scripting engine enhancements
- Workflow: Add event tests

## [0.23.40] - 2025-12-10

### Added
- MCP: Enhance event pipe and workflow events

## [0.23.39] - 2025-12-10

### Added
- Workflow: Add screenshot emit test
- MCP: Add screenshot collection from workflow events with metadata
- Examples: Add strip-ui-styles example for CSS removal
- Core: Focus restore tests and strip-styles improvements

### Changed
- Build: Static VC runtime linking for terminator-nodejs
- Core: Refactor and debug improvements

### Fixed
- Clippy warnings and errors
- Missing trait implementations

## [0.23.38] - 2025-12-10

### Added
- MCP: Add workflow event streaming support with Windows named pipes IPC
- MCP: Add `emit` API to TypeScript workflows for real-time progress updates
- MCP: Add event streaming to `run_command` tool (event_sender, execution_id params)
- SDK: Add `@mediar-ai/workflow` events module with `emit.progress()`, `emit.stepStarted()`, `emit.stepCompleted()`, etc.
- SDK: Add `createStepEmitter()` for scoped event emission with auto-prefixed step context
- Core: Add `get_value()` API for retrieving element values (#407)
- Core: Move `type_into_element` auto-verification to core library (#408)

### Changed
- MCP: Use Windows named pipes instead of stderr JSON parsing for cleaner event IPC
- Build: Bundle bun runtime support for workflows (#407)

## [0.23.36] - 2025-12-04

### Added
- CLI: Add `init` command to scaffold TypeScript workflows
- Tree: Build chained selectors during tree traversal and return in click response
- SDK: Add `geminiComputerUse` to TypeScript SDK
- SDK: Add `processName()` method and document `desktop.applications()`
- Run Command: Default `include_logs` to true for MCP tool
- Run Command: Include partial logs on timeout when `include_logs` is true

### Fixed
- CI: Run all unit tests, exclude e2e/integration/mcp tests that require real desktop
- Browser Script: Improve error message for chrome:// pages
- MCP: Use `eval()` for multi-statement script execution in browser
- Computer Use: Use correct key format `{Ctrl}a` instead of `^a`
- Computer Use: Improve reliability of `type_text_at` and screenshot timing
- Recorder: Drill through containers with broken bounds to find named elements
- Build: Add `opt-level=1` to dev-release profile for faster compilation

### Changed
- Recorder: Change URL search logs from warn to info

### Documentation
- Run Command: Add TypeScript SDK API documentation
- Run Command: Fix incorrect `desktop.locator()` scoping documentation

## [0.23.35] - 2025-12-03

### Added
- Workflow SDK: Add `complete()` function for early workflow exit with success
- Workflow SDK: Add `PendingAction` event for immediate modal display
- Workflow Recorder: Add URL capture for Gemini Computer Use API compliance
- Workflow Recorder: Add UIA debug logging and `page_url` for click events

### Changed
- Workflow SDK: Replace `throw complete()` pattern with `return success()` for cleaner early exit
- Workflow Recorder: Use magenta color key and Consolas font for overlay labels

### Fixed
- Workflow SDK: Use 1-based step index in validation error messages
- Workflow SDK: Improve validation error messages with step index and execution range
- Workflow Recorder: Skip elements with relative timestamps in selectors
- Workflow Recorder: Filter internal browser elements from selector chain
- Workflow Recorder: Stop parent hierarchy at application window boundary
- Workflow Recorder: Improve selector parsing and chain generation
- Workflow Recorder: Use UTF-8 safe string truncation in overlay labels
- MCP Agent: Always inject `accumulated_env` for `run_command` steps
- MCP Agent: Improve `get_window_tree` tool description with vision options

### Tests
- Workflow SDK: Add unit tests for `complete()` early exit functionality

## [0.23.34] - 2025-12-02

### Added
- Workflow SDK: Add `SuccessResult` type and rename `human` to `summary`
- MCP Agent: Add `gemini_computer_use` tool for agentic desktop automation using native Gemini 2.5 Computer Use API
- MCP Agent: Add dynamic MCP tools list to `execute_sequence` instructions
- MCP Agent: Auto-initialize KV variable when `ORG_TOKEN` is present

### Changed
- MCP Agent: Rewrite `gemini_computer_use` to use native Gemini 2.5 Computer Use API
- UI Overlay: Simplify overlay labels to render inside element boxes

### Performance
- MCP Agent: Add granular PERF logs for MCP tool execution timing
- Development: Optimize dev build profile

### Fixed
- MCP Agent: Change premature success log to debug level in `browser_script`

## [0.23.33] - 2025-12-02

### Added
- Workflow SDK: Add `onSuccess` handler support for direct pattern (steps array)
- Workflow SDK: Add `lastStepId` and `lastStepIndex` to `WorkflowSuccessContext`

### Changed
- Style: Run cargo fmt on Rust codebase

### Fixed
- MCP Agent: Optimize window restoration to only restore modified windows

### Performance
- MCP Agent: Add PERF timing logs for MCP tool execution breakdown

### Tests
- Workflow SDK: Add comprehensive tests for onSuccess handler (14 tests)
- MCP Agent: Add array indexing tests for variable substitution

## [0.23.32] - 2025-12-01

### Fixed
- MCP Agent: Strip stack traces from error responses
- TypeScript: Add type assertions to HTTP adapter for strict mode

## [0.23.31] - 2025-12-01

### Added
- MCP Agent: Add KV storage support for run_command scripts
- Docs: Add KV storage documentation to run_command tool

### Fixed
- OCR: Apply DPI scaling to OCR, Omniparser, and Gemini Vision bounds

## [0.23.30] - 2025-11-29

### Added
- Workflow SDK: Add `retry()` function that can be thrown from `execute()` to re-run step
- CI: Add workflow package type checking and unit tests

### Fixed
- Workflow SDK: Fix test type annotations (remove `as any` casts)
- MCP Agent: Pass execution_id as structured tracing field instead of message prefix
- MCP Agent: Use SendMessageTimeoutW to avoid blocking on hung windows
- Recorder: Use `&&` syntax instead of pipe for selectors
- Recorder: Use `text:` instead of `name:contains:` in selectors

### Changed
- KV: Rename VM_TOKEN to ORG_TOKEN in HTTP adapter

## [0.23.29] - 2025-11-28

### Added
- Workflow SDK: Add `retries` option to createStep with configurable delay
- MCP Agent: Add `include_browser_dom` parameter to get_window_tree
- MCP Agent: Add index-based clicking for browser DOM elements (`click_index` with `vision_type: 'dom'`)
- MCP Agent: Add index-based clicking for UI tree elements (`click_index` with `vision_type: 'uia'`)
- MCP Agent: Add `show_overlay` parameter for visual UI debugging
- MCP Agent: Add `browser_dom_max_elements` parameter to get_window_tree
- MCP Agent: Add compact YAML format for omniparser output
- Inspect Overlay: Add display modes, collision detection, and smaller font
- Inspect Overlay: Show text content in DOM overlay labels

### Fixed
- Code: Add missing `include_all_bounds` field and fix clippy warnings
- Inspect Overlay: Use non-blocking polling loop and cache-based UI tree overlay
- Inspect Overlay: Fix DPI scaling for browser DOM
- Inspect Overlay: Improve visual appearance and fix hide functionality
- Inspect Overlay: Simplify label to show only index number

### Changed
- Docs: Update selector syntax to use `&&` instead of legacy pipe `|`

## [0.23.28] - 2025-11-27


### Added
- CI: Publish terminator-cli to crates.io on release (`cargo install terminator-cli`)
- Docs: Add remote-mcp skill for controlling remote machines via MCP

### Fixed
- Code: Resolve clippy warnings (dead_code, format strings)

## [0.23.27] - 2025-11-27

### Added
- MCP Agent: Add configurable timeout for run_command execution
- MCP Agent: Optimize OmniParser image processing for faster inference

### Fixed
- CLI: Fix authentication on remote MCP

## [0.23.26] - 2025-11-26

### Added
- MCP Agent: Add telemetry to TypeScript workflow execution

## [0.23.25] - 2025-11-26

### Added
- Workflow SDK: Add `next` pointer for step branching and loops
  - Static jumps: `next: 'step_id'`
  - Dynamic branching: `next: ({ context }) => condition ? 'a' : 'b'`
  - Retry loops with counter
  - Infinite loop detection (max 1000 iterations)
- Workflow SDK: Auto-read name/version/description from package.json

### Changed
- Workflow SDK: Remove name/version/description from createWorkflow() - now exclusively read from package.json (single source of truth)

### Fixed
- CLI: Remove unused telemetry receiver
- Code: Fix clippy warnings (unused imports, extra blank lines)

## [0.23.24] - 2025-11-26

### Added
- MCP: Stream TypeScript workflow logs through Rust tracing with full OpenTelemetry integration
- MCP: Add log level prefixes ([ERROR], [WARN], [INFO], [DEBUG]) in TypeScript execution
- MCP: Add ParsedLogLine struct and parse_log_line function for log parsing

### Fixed
- Telemetry: TypeScript workflow logs now include trace_id/execution_id for ClickHouse correlation

## [0.23.23] - 2025-11-26

### Added
- MCP: Add unified click_cv_index tool with vision_type parameter (ocr/omniparser) replacing separate click tools
- MCP: Add click_type parameter (left/double/right) to click_cv_index for different click actions
- MCP: Add omniparser support via Replicate API integration
- MCP: Support nested execute_sequence calls in dispatch_tool
- UI Automation: Add click_at_coordinates_with_type method supporting left, double, and right clicks

### Fixed
- Telemetry: Propagate tracing spans to spawned tasks for proper trace correlation
- Telemetry: Include execution_id and trace_id in log body for ClickHouse filtering
- Telemetry: Downgrade expected failures from error! to warn!/debug!
- Code: Address clippy warnings (format strings, needless borrow)

### Changed
- MCP: Replace click_ocr_index and click_omniparser_index with unified click_cv_index tool
- Config: Add files.eol setting to enforce LF line endings

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
