# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v1.1.0 - 2026-02-11

### Added
- **Directory parameter with recursive project discovery** - Pass a directory path to testament and it will recursively search for all `.csproj` files, skipping `bin`, `obj`, and hidden directories. When no `.sln` is found in the given directory, testament now searches parent directories up to the git repository root before falling back to `.csproj` discovery.

### Fixed
- **Test execution failing when run from a different directory** - `dotnet test` and `dotnet build` now run from the project's own directory instead of testament's working directory. Previously, running testament with a directory parameter could cause build failures because dependency resolution relied on the current working directory.

## v1.0.2 - 2026-02-10

### Fixed
- **TRX file not found in PR mode** - Fixed `dotnet test` not creating the TRX results file at the expected path. Now uses `--results-directory` to explicitly control TRX output location instead of relying on an absolute `LogFileName` path.
- **Stderr not visible during test execution** - Stderr is now streamed to the output pane in real-time via a separate thread, making build errors and dotnet diagnostics visible. Previously stderr was piped but never read, also risking process deadlock.
- **Unhelpful error on test failure** - When dotnet test fails before producing results (e.g., build error, no matching tests), the error message now includes the exit code instead of a generic file-not-found error. The dotnet command is also logged to the output pane for manual reproduction.

## v1.0.1 - 2026-02-06

### Fixed
- **Test class grouping with duplicate method names** - Tests with the same method name in different classes (e.g., `ShouldInitialise` in both `ClassA` and `ClassB`) are now correctly grouped into their respective classes. Previously, all tests were grouped under whichever class was parsed last due to a HashMap key collision in `build_test_name_map`.
- **Class-level test run result leaking** - Running tests for a single class no longer incorrectly applies results to other classes that share method names. Result matching now uses a two-pass approach with consumed-result tracking to prevent one TRX result from matching multiple tests.
- **Re-run failed tests matching** - Re-running failed tests no longer incorrectly marks tests in other classes as running when they share method names.

### Added
- Status bar now shows "Running tests..." or "Building..." in the bottom right during execution, returning to "Ready" on completion.

## v1.0.0 - 2026-02-05

Official release.

### Added
- **Discovery caching** - Test discovery results are cached in temp directory, keyed by csproj modification time. Subsequent runs skip expensive `dotnet test --list-tests` when project unchanged.
- **Context header** - TUI now displays "Running Tests for Solution: X.sln" or "Running Tests for PR #123" at the top
- **PR mode improvements** - Now loads only changed projects and filters to only changed tests (not all tests in affected projects)

### Changed
- **Performance optimizations**:
  - Line wrap calculations cached and invalidated only on output change
  - Filter matching precomputes lowercase once per render instead of per test
  - Vector pre-allocation in `build_test_items()` with capacity estimation
- Progress bar now displays inline with "Running tests..." message
- Test count messages simplified to "Tests found in class: N" format

## v0.5.0 - 2026-02-06

### Added
- **PR test runner** - New `testament pr <url>` command to run only tests added/modified in a GitHub PR:
  - Parses PR URL to extract owner/repo/number
  - Fetches PR diff from GitHub API (supports GITHUB_TOKEN or `gh auth token`)
  - Extracts test methods from diff using pattern matching (supports xUnit, NUnit, MSTest)
  - Runs matching tests with `dotnet test --filter`
  - `--no-tui` flag to run tests directly without launching the TUI

### Changed
- **Alphabetical sorting** - Tests panel now displays classes and tests sorted alphabetically (case-insensitive)
- **Per-project collapse state** - Expanding/collapsing test classes is now independent per project; previously "Uncategorized" and other classes shared collapse state across all projects

### Fixed
- **Auto-scroll in Output panel** - Output panel now correctly auto-scrolls when new lines are added

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
- Left/Right arrow keys jump to previous/next test group (class)
- Test group status indicators showing aggregate pass/fail status for each class

### Changed
- Right panel split into Output (top 50%) and Test Result (bottom 50%)
- Groups in Tests pane now start collapsed by default
- Collapse icons changed from v/> to +/- for better visibility
- Status indicator shows "Discovering tests..." instead of "Discovering..."
- Empty class names display as "Uncategorized"
- `r` now runs the test under cursor (or class tests if class selected, or selected tests if any are multi-selected)
- TRX parser now captures error messages and stack traces from failed tests

### Fixed
- Auto-scroll in output pane now waits for actual panel dimensions before scrolling
- Discovery reliability improved with --no-build fallback to avoid parallel build conflicts

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
  - Space to toggle collapse (on class) or select (on test)
  - c to clear selection
  - / to filter tests by name
  - Focused pane highlighting
  - Watch mode (w key) with file system notifications
  - Re-run failed tests (a key)
- Basic TUI with three-pane layout (projects, tests, output)
- Test discovery from .sln files and csproj patterns
- Test execution with TRX parsing
- Real-time output streaming
- README.md with project overview

### Fixed
- Solution search now only looks in current directory (not parent directories)
