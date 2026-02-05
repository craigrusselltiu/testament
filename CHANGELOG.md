# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.4.0 - 2026-02-05

### Added
- **Test Result panel** - New panel showing details of the currently selected test:
  - Test name, status (PASSED/FAILED/SKIPPED/RUNNING/NOT RUN), and duration
  - Error message and stack trace for failed tests (scrollable)
  - Updates in real-time as you navigate between tests
  - Shows "No test selected." when no test is highlighted
- Four-pane layout: Projects | Tests | Output (50%) / Test Result (50%)
- Tab navigation now cycles through all four panes: Projects -> Tests -> Output -> TestResult -> Projects
- Arrow key scrolling in Test Result pane when focused
- `R` (Shift+R) keybinding to run all tests in the project

### Changed
- Right panel split into Output (top 50%) and Test Result (bottom 50%)
- Groups in Tests pane now start collapsed by default
- Collapse icons changed from v/> to +/- for better visibility
- Status indicator shows "Discovering tests..." instead of "Discovering..."
- Empty class names display as "Uncategorized"
- `r` now runs the test under cursor (or class tests if class selected, or selected tests if any are multi-selected)

### Fixed
- Auto-scroll in output pane now waits for actual panel dimensions before scrolling

## v0.3.5 - 2026-02-05

### Changed
- **Instant startup with lazy test discovery** - TUI now launches immediately showing project names, while tests are discovered in the background. Projects show "(...)" while discovering, then update to show test count when ready. Startup phrase displays during discovery, ready phrase appears when complete.
- Parallel test discovery - `dotnet test --list-tests` runs concurrently for all test projects

### Added
- **Test class grouping via C# source parsing** - Tests are now properly grouped by their containing class using tree-sitter to parse C# source files. This enables collapsible class groups in the Tests pane (press Space to toggle). Previously, tests appeared as a flat list because `dotnet test --list-tests` only outputs method names without class information.

## v0.3.3 - 2026-01-31

### Fixed
- Running tests with an active filter now only runs the filtered tests (not all tests)
- If tests are selected with a filter active, only the selected tests run

## v0.3.2 - 2026-01-31

### Added
- Auto-scroll in output pane - output now automatically scrolls to show latest content

### Fixed
- Running selected tests no longer marks unselected tests as Passed (now uses `dotnet test --filter`)
- Re-running failed tests now only runs the failed tests (now uses `dotnet test --filter`)
- Removed duplicate "Running tests..." and "Building..." messages in output

## v0.3.1 - 2026-01-31

### Added
- `x` keybinding to clear output window
- Progress bar during test runs showing completion status (e.g., `[████░░░░] 5/10`)
- StatusDemoTests.cs example file demonstrating passing, failing, and skipped tests

### Changed
- Output window now appends instead of replacing on each run (use `x` to clear)
- Space key now toggles collapse/expand when a class is selected
- Simplified status messages ("Running tests..." instead of "Running all tests for ProjectName...")
- Status bar updated: "Space:toggle" instead of "Space:select"

### Removed
- `j`/`k` vim-style navigation (use arrow keys)
- `h`/`l` vim-style collapse/expand (use Space on class)
- Results summary line after test completion (redundant with test tree display)

## v0.3.0 - 2026-01-30

### Added
- Random startup phrase displayed in output pane on launch (thematic messages like "Gathering the witnesses...", "Let there be tests...")

## v0.2.1 - 2026-01-30

### Changed
- Remove all compiler warnings by removing unused code
- Remove config module (will be re-added when .testament.toml feature is complete)

### Removed
- Unused `SolutionParse` error variant
- Unused `run_tests` synchronous function
- Unused `bg` field from Theme struct
- Unused imports

### Fixed
- Fix `test_no_solution_found_display` test to match updated error message
- Remove obsolete `test_find_solution_in_parent_directory` test (parent directory search was intentionally removed)

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
