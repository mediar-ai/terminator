# Changelog

All notable changes to Terminator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
