# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## v0.2.0 - 2026-01-30

### Added
- Separate build command ('b' key) to build without running tests
- Status messages during build and test execution ("Building...", "Running tests...")
- Arrow key scrolling in the output pane

### Changed
- Test output now filters verbose build logs for cleaner display
- Build output only shown when build fails

## v0.1.0 - 2026-01-30

### Added
- File size guidelines to CLAUDE.md
- Comprehensive unit test suite with 90% coverage
- Core features (Phase 2):
  - Tab/Shift-Tab to switch between panes
  - h/l to collapse/expand test classes
  - Space to toggle test selection
  - c to clear selection
  - / to filter tests by name
  - Focused pane highlighting
  - Watch mode (w key) with file system notifications
  - .testament.toml configuration support
  - Re-run failed tests (a key)
- Basic TUI with three-pane layout (projects, tests, output)
- Test discovery from .sln files and csproj patterns
- Test execution with TRX parsing
- Real-time output streaming
- README.md with project overview

### Fixed
- Solution search now only looks in current directory (not parent directories)
