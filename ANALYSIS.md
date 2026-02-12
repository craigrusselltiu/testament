# Testament - Comprehensive Repository Analysis

## Project Summary

Testament is a Rust-based Terminal User Interface (TUI) application for discovering, running, and monitoring .NET tests. It wraps the `dotnet test` CLI with an interactive four-pane interface, providing real-time output streaming, test class grouping via C# source parsing, watch mode, discovery caching, and a GitHub PR test runner. The project is at version 1.1.0, with Phase 2 (Core Features) mostly complete.

**Codebase statistics:** 23 source files, ~6,100 lines of Rust (including tests), across 7 modules.

---

## Architecture Overview

The application follows a modular architecture with clear separation of concerns:

```
src/
  main.rs          (252 lines) - Entry point, CLI dispatch, PR mode orchestration
  cli.rs           (45 lines)  - Clap-based CLI argument definitions
  app.rs           (888 lines) - Main event loop, keyboard handling, state transitions
  error.rs         (207 lines) - Error types with thiserror
  model/           - Data structures (TestProject, TestClass, Test, TestStatus)
  runner/          - Test execution engine (discovery, executor, watcher)
  parser/          - TRX result parsing and C# source parsing
  ui/              - Ratatui-based TUI components (layout, projects, tests, output, test_result, theme)
  git/             - GitHub PR integration (diff fetching, test extraction)
```

The application uses a synchronous event loop pattern (not async/Tokio despite the dependency listing). The main loop polls for three event sources: keyboard input via crossterm, background discovery events via `mpsc::Receiver`, and test executor events via `mpsc::Receiver`. Background work (discovery, test execution) runs on OS threads, communicating results back to the main thread through channels.

---

## Feature Analysis

### 1. Project Discovery

**Files:** `runner/discovery.rs`, `main.rs`

Testament supports three entry points for locating .NET test projects:

- **Solution file (`.sln`)**: Parses `Project(...)` lines from the solution file, filtering to projects whose names end with `Tests` or `Test`. Path separators are normalized per platform.
- **Project file (`.csproj`)**: Directly uses a single csproj as a test project.
- **Directory**: Searches the given directory and parent directories (up to the git repository root, detected by `.git` presence) for a `.sln` file. If no solution is found, recursively searches for all `.csproj` files, skipping `bin`, `obj`, and hidden directories.

Windows UNC paths (`\\?\`) are stripped before passing to `dotnet` CLI since it does not handle them well.

### 2. Lazy Test Discovery with Background Threading

**Files:** `runner/discovery.rs`, `app.rs`

The TUI launches instantly, showing project names with `(...)` placeholder counts. Test discovery runs in background threads:

1. `discover_projects_lazy()` creates `TestProject` structs immediately (no tests yet) and returns them along with a `mpsc::Receiver<DiscoveryEvent>`.
2. A coordinator thread spawns one child thread per project.
3. Each child thread runs two tasks concurrently via `std::thread::scope`: tree-sitter C# parsing (`build_test_name_map`) and `dotnet test --list-tests`.
4. Results arrive as `DiscoveryEvent::ProjectDiscovered(idx, classes)` messages, updating the UI incrementally.
5. `DiscoveryEvent::Complete` signals all projects are discovered.

The `--no-build` flag is used during discovery to avoid triggering builds; if the project has not been built, discovery reports an error rather than building implicitly.

### 3. Test Class Grouping via C# Source Parsing

**Files:** `parser/csharp.rs`, `runner/discovery.rs`

Since `dotnet test --list-tests` only outputs method names without class information, Testament uses tree-sitter with the C# grammar to parse source files and build a method-to-class mapping:

1. `build_test_name_map()` walks all `.cs` files in the project directory (skipping `bin`, `obj`, hidden directories).
2. Files that don't contain the string "Test" are skipped as a pre-filter.
3. A single tree-sitter `Parser` instance is reused across all files for efficiency.
4. The parser collects ALL methods (not just attributed test methods), since `dotnet test --list-tests` is the authoritative source of which methods are tests.
5. The resulting `HashMap<String, Vec<TestMethodInfo>>` is keyed by both fully-qualified name and bare method name, with each entry containing a `Vec` to handle methods with the same name in different classes.
6. `group_tests_by_class()` correlates the `dotnet test --list-tests` output with the name map, using a `used_counts` tracker to cycle through duplicate method name entries, preventing HashMap key collisions.

Supports both block-scoped and file-scoped C# namespace declarations, and nested namespaces.

### 4. Discovery Caching

**Files:** `runner/discovery.rs`

Test discovery results are cached in the system temp directory to avoid expensive `dotnet test --list-tests` calls on subsequent runs:

- Cache key: A hash of the project path, stored at `testament_discovery_{hash}.cache`.
- Cache validity: Keyed by the maximum modification time of the `.csproj` file and the newest `.dll` in the `bin/` directory (recursive). This ensures rebuilds invalidate the cache even when the csproj itself hasn't changed.
- Cache format: First line is the mtime, followed by one test name per line.

### 5. Four-Pane TUI Layout

**Files:** `ui/layout.rs`, `ui/projects.rs`, `ui/tests.rs`, `ui/output.rs`, `ui/test_result.rs`

The interface is divided into four panes:

| Pane | Position | Size | Purpose |
|------|----------|------|---------|
| Projects | Left | 20% width | List of test projects with test counts |
| Tests | Middle | 40% width | Collapsible test classes with individual tests |
| Output | Right top | 40% width, 50% height | Streaming test output and build results |
| Test Result | Right bottom | 40% width, 50% height | Details of the selected test |

An optional context header bar displays "Running Tests for Solution: X.sln" or "Running Tests for PR #123" at the top. A status bar at the bottom shows keybindings on the left and current status (Ready/Running tests.../Building.../Discovering tests...) on the right.

### 6. Test Navigation and Interaction

**Files:** `app.rs`, `ui/tests.rs`

The Tests pane displays classes as collapsible groups with tests nested underneath:

- **Collapse/Expand**: `Space` on a class header toggles collapse. `c` toggles expand/collapse for all classes in the current project. Classes start collapsed by default.
- **Collapse state is per-project**: Uses composite keys of format `"{project_name}::{class_full_name}"` to keep collapse state independent between projects.
- **Multi-select**: `Space` on a test toggles selection (shown as `[x]`/`[ ]`). `C` clears all selections.
- **Group navigation**: `Left`/`Right` arrow keys jump to the previous/next class header, wrapping around.
- **Status indicators**: Each class shows an aggregate status icon (`+` passed, `x` failed, `*` running, `-` skipped, ` ` not run). Individual tests show the same icons.
- **Alphabetical sorting**: Classes are pre-sorted by `full_name_lower` and tests by `name_lower` at discovery time.

### 7. Test Filtering

**Files:** `app.rs`, `ui/tests.rs`

Pressing `/` enters filter mode. The filter is case-insensitive and matches against test names:

- Classes with no matching tests are hidden entirely.
- The filter text is lowercased once per render cycle (not per test).
- Running tests with an active filter only runs the filtered tests.
- `Enter` applies the filter, `Esc` clears it.
- Both `Test` and `TestClass` `name_lower` fields are precomputed at construction time for efficient lowercase comparison.

### 8. Test Execution

**Files:** `runner/executor.rs`, `app.rs`

Test execution is handled by `TestExecutor`, which runs `dotnet test` in a background thread:

- **Run modes**: Run all tests (`R`), run test under cursor (`r` on a test), run class tests (`r` on a class header), run selected tests (`r` with multi-selection), run filtered tests (`r` with active filter).
- **Filter construction**: Uses `FullyQualifiedName~{name}` filters joined with `|`. Parameterized test arguments (parenthesized) are stripped to avoid MSBuild special character issues.
- **TRX logging**: Uses `--logger trx;LogFileName={temp_path}` with a unique per-run path (PID + nanosecond timestamp) to avoid stale results.
- **Output streaming**: `stdout` is piped through a `BufReader` and lines are sent as `ExecutorEvent::OutputLine` through the channel. Build noise is filtered by `should_show_line()`, which skips MSBuild, NuGet, VSTest, xUnit diagnostic lines, stack traces, and build output paths.
- **Stderr**: Piped to null in the executor (stderr handling was improved in v1.0.2 based on changelog, though the current implementation pipes stderr to null for the test runner).
- **Progress tracking**: Passed/Failed lines from dotnet output increment a progress counter displayed as a bar in the output pane: `[████░░░░] 5/10`.
- **Build-only mode**: `b` key runs `dotnet build --verbosity minimal` and shows output only on failure.
- **Working directory**: Both `dotnet test` and `dotnet build` use `current_dir(project_dir)` to run from the project's own directory, fixing dependency resolution issues.

### 9. TRX Result Parsing

**Files:** `parser/trx.rs`

The TRX parser uses `quick-xml` to extract test results from Visual Studio Test Results XML files:

- Handles both self-closing (`<UnitTestResult ... />`) and content-bearing (`<UnitTestResult>...</UnitTestResult>`) elements.
- Extracts: `testName`, `outcome` (Passed/Failed/anything else maps to Skipped), `duration` (HH:MM:SS.FFFFFFF format parsed to milliseconds).
- Captures `ErrorInfo/Message` and `ErrorInfo/StackTrace` for failed tests, combining them into `error_message`.
- Missing `testName` causes the result to be skipped. Missing `outcome` defaults to Passed. Missing `duration` defaults to 0.

### 10. Result Matching (Two-Pass with Consumed Tracking)

**Files:** `app.rs` (`apply_results`)

Matching TRX results back to discovered tests is non-trivial because names may differ between what `dotnet test --list-tests` reports and what appears in TRX:

- **Pass 1**: Tries exact `full_name` match, then suffix match (result's name after the last `.` matches the test's `full_name`).
- **Pass 2**: Falls back to bare `name` match for any tests still in "Running" state.
- **Consumed tracking**: A `consumed` boolean vector prevents one TRX result from matching multiple tests (critical when duplicate method names exist across classes).

### 11. Re-run Failed Tests

**Files:** `app.rs`

Pressing `a` re-runs tests that failed in the last execution:

- `last_failed` HashSet is populated from results with `TestOutcome::Failed`.
- Failed test suffixes are precomputed into a `HashSet<String>` for O(1) lookup.
- Only tests matching the failed set are marked as Running and included in the filter.

### 12. Watch Mode

**Files:** `runner/watcher.rs`, `app.rs`

Pressing `w` toggles watch mode:

- Uses the `notify` crate with `RecommendedWatcher` in recursive mode on the solution directory.
- Filters to `.cs` and `.csproj` file modifications/creations only.
- Implements a 500ms debounce to avoid rapid re-triggers from file save operations.
- On file change, automatically triggers `run_tests()` if no tests are currently executing.
- The `[WATCH]` indicator appears in the status bar when active.

### 13. PR Test Runner

**Files:** `git/pr.rs`, `main.rs`, `app.rs`

The `testament pr <url>` subcommand identifies and runs only tests changed in a GitHub pull request:

**PR URL parsing**: Regex-based extraction of owner, repo, and number from GitHub PR URLs.

**Authentication**: Checks `GITHUB_TOKEN` environment variable first, falls back to `gh auth token` CLI.

**Diff fetching**: Uses `reqwest::blocking::Client` to fetch the PR diff from `https://api.github.com/repos/{owner}/{repo}/pulls/{number}` with the `application/vnd.github.v3.diff` Accept header.

**Test extraction from diff**: Two strategies are used:
1. **Attribute-based**: Looks for lines with test attributes (`[Fact]`, `[Theory]`, `[Test]`, `[TestMethod]`, `[TestCase]`) followed by method declarations within 5 lines.
2. **Naming convention**: Matches method declarations whose names match patterns like `Test*`, `*Test`, `*Tests`, `*Should*`, `Should*`.

Only added lines (starting with `+`) in files detected as test files (path contains "test" or "spec") are analyzed.

**Project resolution**: For each changed test file, walks parent directories to find the containing `.csproj`.

**Two execution modes**:
- **TUI mode** (default): Launches the TUI with only the affected projects loaded. Tests are pre-filtered to changed tests only, auto-selected, and classes expanded.
- **`--no-tui` mode**: Runs `dotnet test --filter` directly in the terminal with output to stdout.

### 14. Context-Aware Test Result Pane

**Files:** `ui/test_result.rs`, `ui/layout.rs`

The Test Result pane adapts based on what is selected:

- **Test selected**: Shows test name, status (PASSED/FAILED/SKIPPED/RUNNING/NOT RUN with colored styling), duration (formatted as ms or seconds), and error message with stack trace for failed tests. Content is scrollable.
- **Class selected**: Shows "Tests found in class: N".
- **Project focused**: Shows "Tests found in project: N".
- **Nothing selected**: Shows "No test selected."

### 15. Themed UI

**Files:** `ui/theme.rs`

The application uses an amber/gold color theme reflecting its biblical naming:

| Element | Color |
|---------|-------|
| Foreground | Amber (RGB 255, 191, 0) |
| Highlight | Gold (RGB 255, 215, 0) |
| Border | Dark gold (RGB 139, 119, 42) |
| Passed | Green |
| Failed | Red |
| Running | Yellow |
| Skipped | Dark gray |

Active panes have highlighted borders; inactive panes use the dark gold border.

### 16. Startup Experience

**Files:** `ui/layout.rs`

On launch, the output pane displays:
- ASCII art of a cross (the biblical theme)
- A random startup phrase from 8 options (e.g., "Gathering the witnesses...", "Let there be tests...")
- After discovery completes, a random ready phrase from 11 options (e.g., "Testament is ready.", "Bear witness.")

The random selection uses nanosecond-precision system time with bit mixing for distribution.

### 17. CLI Interface

**Files:** `cli.rs`

Built with Clap's derive API:

```
testament [PATH]              # Launch TUI (default)
testament run [--filter NAME] # Run tests with optional filter
testament pr <URL> [--path PATH] [--no-tui]  # PR test runner
```

The `PATH` argument accepts a solution file, project file, or directory.

### 18. Error Handling

**Files:** `error.rs`

Uses `thiserror` with four error variants:
- `NoSolutionFound` - No `.sln` or `.csproj` file found
- `FileRead { path, source }` - File I/O errors with context
- `DotnetExecution(String)` - dotnet CLI failures
- `TrxParse(String)` - TRX XML parsing errors
- `Io(std::io::Error)` - General I/O errors (with `From` conversion)

### 19. Output Buffer Management

**Files:** `ui/layout.rs`

The output buffer implements:
- **Append-only**: New output is appended; `x` clears the buffer.
- **Auto-scroll**: When enabled (during test/build execution), automatically scrolls to the bottom.
- **Buffer bounding**: When the buffer exceeds 2,000 lines, it is trimmed to 1,000 lines by removing the oldest content.
- **Newline counting**: Maintained incrementally as text is appended (O(1) per append for scroll bounds).

---

## Performance Improvements

### 1. Dirty Flag Rendering

**Affected area:** Main event loop (`app.rs`)

The TUI only redraws when `state.dirty` is set to `true`. Every state mutation (output append, test status change, pane switch, scroll, etc.) sets the dirty flag. After each render, the flag is cleared. This avoids unnecessary redraws during idle periods.

### 2. Dynamic Poll Timeout

**Affected area:** Main event loop (`app.rs`)

The keyboard input poll timeout adapts to the application state:
- **Active** (discovering or executing): 33ms (~30 FPS) for responsive UI updates.
- **Idle**: 250ms (~4 FPS) to reduce CPU usage when nothing is happening.

### 3. Cached Test Item List

**Affected area:** Test navigation and rendering (`ui/layout.rs`)

The flattened test item list (used for navigation) is cached and only rebuilt when the cache key changes. The key is a tuple of `(project_idx, collapse_generation, filter)`. A `collapse_generation` counter (incremented on each collapse state change) replaces an expensive hash of the entire `collapsed_classes` HashSet.

### 4. Cached Line Wrap Calculations

**Affected area:** Output pane auto-scroll (`ui/layout.rs`)

The `get_total_output_lines()` method caches the total wrapped line count, keyed by `output.len()`. This avoids re-calculating line wrapping on every frame when only the scroll position changes. The cache is invalidated whenever new output is appended.

### 5. Precomputed Lowercase Strings

**Affected area:** Filter matching (`model/test.rs`, `ui/tests.rs`)

- `Test.name_lower` and `TestClass.full_name_lower` are computed once at construction time.
- During rendering, the filter string is lowercased once per `TestList` construction (`filter_lower`), not once per test comparison.
- In `build_test_items()`, the filter is lowercased once before iterating.

### 6. Vector Pre-allocation

**Affected area:** Test list building (`ui/tests.rs`)

`build_test_items()` estimates capacity as `classes.len() + sum(class.tests.len())` and pre-allocates the vector, avoiding repeated reallocations during the build.

### 7. String Buffer Reuse for Collapse Keys

**Affected area:** Test list rendering and navigation (`ui/tests.rs`)

Both `TestList::render()` and `build_test_items()` allocate a single `collapse_key_buf` string with the project prefix, then truncate and reuse it for each class lookup. This avoids allocating a new string per class per frame.

### 8. Pre-sorted Classes and Tests

**Affected area:** Discovery and rendering (`runner/discovery.rs`)

Classes are sorted by `full_name_lower` and tests within each class are sorted by `name_lower` at discovery time. This eliminates sorting during rendering, which happens every frame.

### 9. Parallel Test Discovery

**Affected area:** Startup (`runner/discovery.rs`)

All test projects run `dotnet test --list-tests` concurrently on separate threads. Within each project, tree-sitter parsing and dotnet list-tests run concurrently using `std::thread::scope`. This parallelism significantly reduces startup time for multi-project solutions.

### 10. Tree-sitter Parser Reuse

**Affected area:** C# source parsing (`parser/csharp.rs`)

A single `Parser` instance is created once in `build_test_name_map()` and reused for all `.cs` files in a project. This avoids the overhead of repeated parser initialization and language loading.

### 11. Pre-filter for C# Files

**Affected area:** C# source parsing (`parser/csharp.rs`)

Files that don't contain the string "Test" are skipped entirely before parsing. This avoids running the tree-sitter parser on non-test source files that would contribute no useful information.

### 12. Discovery Caching with Smart Invalidation

**Affected area:** Test discovery (`runner/discovery.rs`)

Cached discovery results avoid the expensive `dotnet test --list-tests` call entirely on subsequent runs. The cache is invalidated not just on `.csproj` changes but also on DLL changes (by checking the newest `.dll` mtime in the `bin/` directory), catching rebuilds that don't modify the project file.

### 13. Incremental Newline Counting

**Affected area:** Output buffer management (`ui/layout.rs`)

`output_newline_count` is maintained incrementally: each `append_output()` call counts only the newlines in the appended text, avoiding a full scan of the output buffer. This provides O(n) cost relative to the new text, not the total buffer size.

### 14. HashSet-based Failed Test Suffix Lookup

**Affected area:** Re-run failed tests (`app.rs`)

When re-running failed tests, method name suffixes are precomputed into a `HashSet<String>` for O(1) lookup instead of performing string operations per test during the marking phase.

### 15. Lazy Regex Compilation

**Affected area:** PR mode (`git/pr.rs`)

All four regex patterns used for PR diff parsing are compiled once using `LazyLock` (static initialization), avoiding repeated regex compilation on each PR analysis call.

---

## Data Flow

```
User starts testament
  |
  v
find_solution() --> parse_solution() --> discover_projects_lazy()
  |                                           |
  v                                           v
TUI launches instantly             Background threads: per project
  |                                    |                |
  v                                    v                v
AppState::new()              tree-sitter parse   dotnet test --list-tests
  |                                    |                |
  v                                    +--------+-------+
Event loop polls:                               |
  - Keyboard (crossterm)                        v
  - DiscoveryEvent (mpsc)           group_tests_by_class()
  - ExecutorEvent (mpsc)                        |
  |                                             v
  v                             DiscoveryEvent::ProjectDiscovered
User presses 'r'                        |
  |                                     v
  v                             UI updates test list
TestExecutor::run()
  |
  v
Background thread:
  dotnet test --logger trx
  |
  +-- stdout --> OutputLine events --> progress bar
  |
  v
  Parse TRX file
  |
  v
  ExecutorEvent::Completed(results)
  |
  v
  apply_results() (two-pass matching)
  |
  v
  UI updates test statuses
```

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| ratatui | 0.29 | Terminal UI framework |
| crossterm | 0.28 | Cross-platform terminal manipulation |
| clap | 4 (derive) | CLI argument parsing |
| quick-xml | 0.37 | TRX test result file parsing |
| thiserror | 2 | Derive macro for error types |
| notify | 7 | File system watching for watch mode |
| tree-sitter | 0.24 | Incremental parsing framework |
| tree-sitter-c-sharp | 0.23 | C# grammar for tree-sitter |
| reqwest | 0.12 (blocking) | HTTP client for GitHub API |
| regex | 1 | Pattern matching for PR diff parsing |
| tempfile | 3 (dev) | Temporary files for tests |

---

## Test Coverage

The codebase includes unit tests in most modules:

- **error.rs**: Tests for all error variants, Display/Debug formatting, From conversions
- **model/project.rs**: Tests for project creation, test counting, cloning, edge cases
- **model/test.rs**: Tests for status equality, test creation, mutation, class construction
- **parser/trx.rs**: Tests for TRX parsing including edge cases (empty, malformed, missing attributes, special characters)
- **parser/csharp.rs**: Tests for xUnit, NUnit, MSTest attribute parsing, namespaces, file-scoped namespaces
- **runner/discovery.rs**: Tests for project name detection, test grouping, solution parsing, csproj finding
- **ui/tests.rs**: Tests for test list building, collapsing, filtering, edge cases
- **ui/layout.rs**: Tests for AppState, pane navigation, collapse toggling, output management
- **git/pr.rs**: Tests for PR URL parsing, test file detection, diff extraction
