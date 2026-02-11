# Testament - Specification

*A Rust TUI for running .NET tests*

## Overview

Testament is a terminal user interface for discovering, running, and monitoring .NET tests. The name evokes both "test" and the idea of a canonical record—your tests as the authoritative witness to your code's correctness.

The biblical theme is kept subtle: a cross in the header, warm amber/gold accent colors, and the occasional turn of phrase in status messages. The interface itself remains entirely conventional and professional.

## Project Discovery

Testament automatically discovers test projects by searching for a solution file (`.sln`) in the current directory or any parent directory. If found, it parses the solution to identify all test projects. If no solution is found, it falls back to scanning for `*.csproj` files matching common test project patterns.

### Discovery Order

1. Look for `.testament.toml` in current directory (may specify explicit paths)
2. Search current directory and parent directories (up to git repo root) for `.sln` file, parse for test projects
3. Fall back to recursive search for `*.csproj` files in the given directory

### Test Project Detection

A project is considered a test project if it:

- Has a name matching `*Tests` or `*Test`
- References a known test framework package (xUnit, NUnit, MSTest)
- Contains files with test attributes

```rust
pub struct Discovery {
    pub root: PathBuf,
    pub solution: Option<PathBuf>,
    pub projects: Vec<TestProject>,
}
```

### Test Method Discovery

Testament uses two different mechanisms for discovering individual test methods:

| Mechanism | Used For | How It Works |
|-----------|----------|--------------|
| `dotnet test --list-tests` | Normal operation | Authoritative list from the test framework itself. Handles parameterized tests, generated tests, and all edge cases correctly. |
| Tree-sitter C# parsing | PR mode only | Parses source diffs to identify added/modified test methods without invoking dotnet. Fast but heuristic-based. |

For normal test discovery and execution, Testament always uses `dotnet test --list-tests` as the source of truth. Tree-sitter parsing is only used in PR mode to analyze git diffs and identify which test methods were added or modified, enabling Testament to run only the relevant tests without needing to invoke dotnet on every commit in the diff.

## Test Framework Support

Testament supports the three major .NET test frameworks equally:

| Framework | Test Attributes                            | Setup/Teardown                    |
|-----------|--------------------------------------------|-----------------------------------|
| xUnit     | `[Fact]`, `[Theory]`                       | Constructor / `IDisposable`       |
| NUnit     | `[Test]`, `[TestCase]`, `[TestCaseSource]` | `[SetUp]`, `[TearDown]`           |
| MSTest    | `[TestMethod]`, `[DataTestMethod]`         | `[TestInitialize]`, `[TestCleanup]` |

Framework detection is automatic based on package references in the `.csproj` file. No configuration needed.

### Parameterized Tests

Parameterized tests (`[Theory]`, `[TestCase]`, `[DataTestMethod]`) generate multiple test instances. Testament displays these as expandable children under the parent method:

```
▼ AuthTests
  ▼ ValidateToken (3 cases)
      ✓ ValidateToken(token: "valid", expected: true)
      ✓ ValidateToken(token: "expired", expected: false)
      ✗ ValidateToken(token: null, expected: false)
```

The parent shows aggregate status: passed if all children pass, failed if any child fails.

## Multi-Target Framework Support

When a test project targets multiple frameworks (e.g., `net8.0;net9.0`), Testament runs tests against all targets by default and groups results by framework:

```
▼ Api.Tests
  ▼ net8.0
      ✓ AuthTests (5)
      ✓ UserTests (3)
  ▼ net9.0
      ✓ AuthTests (5)
      ✗ UserTests (3)
```

Override with `--framework <tfm>` to run against a single target:

```bash
testament run --framework net8.0
```

Configuration option to set a default:

```toml
[runner]
default_framework = "net8.0"  # or "all" (default)
```

## Test Output Streaming

Testament streams test output in real-time as tests execute, rather than waiting for completion. This is achieved by:

1. Running `dotnet test` with `--verbosity normal` and capturing stdout/stderr
2. Parsing incremental output to detect test start/completion events
3. Updating the UI immediately as events arrive

The output pane shows live output for the currently selected (or most recently started) test. Users can scroll through output history and copy to clipboard.

```rust
pub struct OutputBuffer {
    pub lines: VecDeque<OutputLine>,
    pub max_lines: usize,  // Default 10,000
    pub follow: bool,      // Auto-scroll to bottom
}

pub struct OutputLine {
    pub timestamp: Instant,
    pub source: OutputSource,
    pub content: String,
}

pub enum OutputSource {
    Stdout,
    Stderr,
    Testament,  // Internal messages
}
```

## UI Layout

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ✝ TESTAMENT                                         Running: 12/150 ◐   │
├──────────────────────┬─────────────────────────┬─────────────────────────┤
│ Projects             │ Tests                   │ Output                  │
│ ────────             │ ─────                   │ ──────                  │
│ › Api.Tests          │ + AuthTests             │ Running tests...        │
│   Core.Tests         │   ✓ LoginTest           │ Build succeeded         │
│                      │   ✗ LogoutTest          │                         │
│                      │   ◐ RefreshTest         ├─────────────────────────┤
│                      │ + UserTests (4)         │ Test Result             │
│                      │ + OrderTests (12)       │ ───────────             │
│                      │                         │ Test: LoginTest         │
│                      │                         │ Status: PASSED          │
│                      │                         │ Duration: 42ms          │
├──────────────────────┴─────────────────────────┴─────────────────────────┤
│ [r]un  [w]atch  [f]ilter  [a]gain  [q]uit                   48 ✓  2 ✗  1 ○│
└──────────────────────────────────────────────────────────────────────────┘
```

### Four-Pane Layout

1. **Projects** (left, 20%) - List of test projects
2. **Tests** (middle, 40%) - Test classes (collapsible) and test methods
3. **Output** (right top, 40% of right panel) - Build and test execution output
4. **Test Result** (right bottom, 60% of right panel) - Details of selected test

Use `Tab`/`Shift+Tab` to navigate: Projects → Tests → Output → Test Result → Projects

### Status Icons

- `○` skipped
- `◐` running
- `✓` passed
- `✗` failed
- `+` collapsed class
- `-` expanded class

### Color Theme

The default theme uses warm, parchment-inspired tones:

| Element       | Color                   |
|---------------|-------------------------|
| Header/accent | Amber/gold (#D4A574)    |
| Passed        | Soft green (#7D9F6A)    |
| Failed        | Muted red (#C25450)     |
| Running       | Gold (#D4A574)          |
| Skipped       | Gray (#888888)          |
| Background    | Dark warm gray (#1E1E1C)|
| Text          | Off-white (#E8E4D9)     |

Alternative themes: `--theme modern` for conventional blue/green/red.

### Sample Messages

Standard messages with occasional thematic touches:

```
Discovering tests in Api.Tests...
Running 47 tests across 3 projects...
Watching for changes...

RESULTS
───────
Passed:  44
Failed:   2
Skipped:  1
Duration: 3.2s

2 tests await redemption. Press 'a' to run failed.
```

## CLI Interface

```bash
testament                    # Launch TUI
testament run                # Run all tests
testament run -p Api.Tests   # Run specific project
testament run -f "Auth*"     # Filter expression
testament list               # List discovered tests
testament watch              # Watch mode
testament again              # Re-run failed tests
testament pr <url|number>    # Run tests changed in a PR
testament bisect <test>      # Find commit where test started failing
testament init               # Generate starter .testament.toml
testament --help
testament --version
```

### Flags

```
-p, --project <NAME>     Run specific project
-c, --class <NAME>       Run specific test class
-t, --test <NAME>        Run specific test
-f, --filter <EXPR>      dotnet test filter expression
    --framework <TFM>    Target framework (e.g., net8.0)
    --parallel <N>       Max parallel project runs (default: auto)
    --no-build           Skip build
    --theme <NAME>       Color theme (default, modern)
    --verbose            Detailed output
    --json               Output results as JSON
    --version            Show version information
```

### Parallelization

The `--parallel <N>` flag controls how many test **projects** run concurrently:

- `--parallel 1` - Run projects sequentially (useful for debugging or resource-constrained environments)
- `--parallel 4` - Run up to 4 projects simultaneously
- `--parallel auto` (default) - Use number of CPU cores / 2, minimum 1

Parallelization of tests *within* a project is controlled by the test framework itself (via `.runsettings` or framework-specific configuration). Testament does not override this behavior.

```toml
# .testament.toml
[runner]
parallel = 4  # or "auto"
```

## Error Handling

Testament handles errors gracefully with clear user feedback. Errors are displayed in the output pane with suggested remediation.

### Build Failures

When `dotnet build` fails:

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ✝ TESTAMENT                                              Build Failed    │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   BUILD FAILED                                                           │
│   ────────────                                                           │
│                                                                          │
│   Api.Tests failed to build:                                             │
│                                                                          │
│   src/AuthService.cs(42,17): error CS1002: ; expected                    │
│   src/AuthService.cs(43,1): error CS1519: Invalid token                  │
│                                                                          │
│   Fix the build errors and press 'r' to retry.                           │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│ [r]etry  [q]uit                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

### Project Load Failures

If a test project cannot be loaded (missing dependencies, invalid csproj):

- The project appears in the list with a ⚠ warning icon
- Selecting it shows the error details in the output pane
- Other projects remain runnable

### Network Errors (PR Mode)

When GitHub API is unreachable or rate-limited:

```
GitHub API error: rate limit exceeded
Reset at: 14:32:07 (in 12 minutes)

Options:
  • Wait for rate limit reset
  • Set GITHUB_TOKEN for higher limits (5000 req/hr)
  • Use 'gh auth login' to authenticate GitHub CLI
```

### Git Credential Errors

When checkout fails due to authentication:

```
Git authentication failed for 'origin'

Testament tried these credential sources:
  ✗ GITHUB_TOKEN environment variable (not set)
  ✗ GitHub CLI (not authenticated)
  ✗ Git credential helper (no credentials found)

To fix:
  • Run 'gh auth login' to authenticate GitHub CLI
  • Or set GITHUB_TOKEN environment variable
  • Or configure git credentials: git config credential.helper store
```

### Missing Dependencies

```
dotnet SDK not found in PATH

Testament requires the .NET SDK to run tests.
Install from: https://dotnet.microsoft.com/download
```

### Error Data Model

```rust
pub enum TestamentError {
    Build { project: String, output: String },
    ProjectLoad { project: String, reason: String },
    Discovery { path: PathBuf, reason: String },
    Git { operation: String, reason: String },
    GitHub { status: Option<u16>, message: String, retry_after: Option<Duration> },
    DotnetNotFound,
    NetworkUnavailable,
    InvalidConfig { path: PathBuf, reason: String },
}

impl TestamentError {
    pub fn user_message(&self) -> String;
    pub fn suggestion(&self) -> Option<String>;
    pub fn is_retryable(&self) -> bool;
}
```

## Features

### PR Test Runner

Testament can inspect a pull request, identify new or modified tests, and run only those tests locally.

#### Usage

```bash
# By PR URL
testament pr https://github.com/org/repo/pull/123

# By PR number (uses origin remote to infer repo)
testament pr 123

# Run the full project(s) containing changed tests instead of just the tests
testament pr 123 --include-project

# Preview what would run without executing
testament pr 123 --dry-run
```

#### Workflow

```
testament pr 123
        │
        ▼
┌───────────────────────────────────┐
│ Fetch PR metadata from GitHub     │
│ (branch name, changed files)      │
└─────────────────┬─────────────────┘
                  ▼
┌───────────────────────────────────┐
│ Is branch checked out locally?    │
├─────────────┬─────────────────────┤
│     Yes     │         No          │
│      │      │          │          │
│      │      │          ▼          │
│      │      │   git fetch origin  │
│      │      │   git checkout      │
│      │      │   git pull          │
│      │      │   dotnet build      │
└──────┴──────┴─────────────────────┘
                  ▼
┌───────────────────────────────────┐
│ Parse diff for test changes:      │
│ - New test files (*Tests.cs)      │
│ - Modified test methods           │
│ - New [Fact]/[Theory] attrs       │
└─────────────────┬─────────────────┘
                  ▼
┌───────────────────────────────────┐
│ Run identified tests              │
│ (or full projects if --project)   │
└───────────────────────────────────┘
```

#### PR Mode UI

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ✝ TESTAMENT                                    PR #123: Add auth tests   │
├───────────────────────────────────┬──────────────────────────────────────┤
│ Changed Tests (5)                 │ Output                               │
│ ─────────────────                 │ ──────                               │
│ Api.Tests                         │                                      │
│   ✓ AuthTests.LoginTest           │ Checked out: feature/auth            │
│   ✓ AuthTests.LogoutTest          │ Building...                          │
│   ◐ AuthTests.RefreshTest         │                                      │
│ Core.Tests                        │ Running 5 tests from PR #123         │
│   ○ TokenTests.ValidateTest       │                                      │
│   ○ TokenTests.ExpireTest         │                                      │
├───────────────────────────────────┴──────────────────────────────────────┤
│ [r]un  [P]roject  [d]iff  [q]uit                              2 ✓  0 ✗   │
└──────────────────────────────────────────────────────────────────────────┘
```

#### Credential Resolution

Testament attempts to authenticate with GitHub using these sources (in order):

1. `GITHUB_TOKEN` environment variable
2. GitHub CLI (`gh auth token`)
3. Git credential helper

For private repositories, at least one of these must be configured.

#### Test Detection Heuristics

Testament parses the PR diff to identify test changes:

1. **New test files**: Any added file matching `*Tests.cs`, `*Test.cs`, `*Spec.cs`
2. **New test methods**: Added lines containing `[Fact]`, `[Theory]`, `[Test]`, `[TestMethod]`
3. **Modified test methods**: Changes within existing test method bodies
4. **Renamed tests**: Detected via git's rename detection

```rust
pub struct PrTestChange {
    pub file_path: PathBuf,
    pub project: String,
    pub class_name: Option<String>,
    pub method_name: Option<String>,
    pub change_type: ChangeType,
}

pub enum ChangeType {
    NewFile,
    NewMethod,
    ModifiedMethod,
    Renamed { from: String },
}
```

### Test Bisect (Time Travel)

Find the exact commit where a test started failing using binary search across git history.

#### Usage

```bash
# Find when a specific test started failing
testament bisect "Api.Tests.AuthTests.LogoutTest"

# Limit search range
testament bisect "AuthTests.LogoutTest" --good v1.2.0 --bad HEAD

# Bisect with a time range
testament bisect "AuthTests.LogoutTest" --since "2 weeks ago"

# Skip build step (for repos where tests can run without rebuilding)
testament bisect "AuthTests.LogoutTest" --skip-build
```

#### Workflow

```
testament bisect "AuthTests.LogoutTest"
        │
        ▼
┌───────────────────────────────────┐
│ Verify test fails on HEAD         │
└─────────────────┬─────────────────┘
                  ▼
┌───────────────────────────────────┐
│ Find last known good commit       │
│ (binary search backwards or       │
│  use provided --good)             │
└─────────────────┬─────────────────┘
                  ▼
┌───────────────────────────────────┐
│ Binary search between good/bad    │
│                                   │
│  good ◀───────────────────▶ bad   │
│               ▲                   │
│            midpoint               │
│          test here                │
└─────────────────┬─────────────────┘
                  ▼
┌───────────────────────────────────┐
│ Report culprit commit:            │
│ - Hash, author, date              │
│ - Commit message                  │
│ - Changed files                   │
└───────────────────────────────────┘
```

#### Bisect UI

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ✝ TESTAMENT                                              Bisecting...    │
├──────────────────────────────────────────────────────────────────────────┤
│ Target: AuthTests.LogoutTest                                             │
│                                                                          │
│ Search Progress                                                          │
│ ───────────────                                                          │
│                                                                          │
│   a1b2c3d ✓ ──────────────────────────────────────────── e5f6g7h ✗       │
│               │       │       │       │       │                          │
│               ✓       ✓       ◐       ?       ?                          │
│                               │                                          │
│                            testing                                       │
│                                                                          │
│ Step 4/7 · Commit: d4e5f6g · Building...                                 │
│                                                                          │
│ "Add caching to auth service"                                            │
│ Author: jane@ · 3 days ago                                               │
├──────────────────────────────────────────────────────────────────────────┤
│ [s]kip commit  [q]uit                                     ETA: ~2 min    │
└──────────────────────────────────────────────────────────────────────────┘
```

#### Bisect Result

```
BISECT COMPLETE
───────────────

Test "AuthTests.LogoutTest" started failing at:

  Commit:  d4e5f6a
  Author:  jane@example.com
  Date:    2024-01-15 14:32:07
  Message: Add caching to auth service

  Files changed:
    M src/Api/Services/AuthService.cs
    M src/Api/Cache/TokenCache.cs

  Parent (last passing): c3d4e5f "Fix token refresh logic"

Tested 7 commits in 3m 42s.
```

#### Cancellation and Cleanup

If the user quits mid-bisect (`q` or Ctrl+C), Testament:

1. Restores the original branch/commit that was checked out before bisect started
2. If there were uncommitted changes, they are preserved via `git stash` at the start and restored on exit
3. Cleans up any temporary TRX files

```rust
pub struct BisectSession {
    pub target_test: String,
    pub good_commit: String,
    pub bad_commit: String,
    pub current_commit: String,
    pub original_ref: String,          // Branch or commit to restore
    pub stashed_changes: bool,         // Whether we stashed uncommitted work
    pub history: Vec<BisectStep>,
    pub status: BisectStatus,
}
```

#### Bisect Data Model

```rust
pub struct BisectStep {
    pub commit: String,
    pub result: Option<BisectResult>,
    pub duration: Option<Duration>,
}

pub enum BisectResult {
    Passed,
    Failed,
    Skipped,  // Build failed, test not found, etc.
}

pub enum BisectStatus {
    Testing,
    Building,
    Found { culprit: CommitInfo },
    Inconclusive { reason: String },
}

pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: DateTime<Utc>,
    pub message: String,
    pub files_changed: Vec<PathBuf>,
}
```

## Configuration

`.testament.toml`:

```toml
[general]
parallel = true
timeout = 300

[discovery]
# Explicit project paths (optional, overrides auto-discovery)
# projects = ["tests/Api.Tests", "tests/Core.Tests"]

[watch]
debounce_ms = 500
patterns = ["**/*.cs", "**/*.csproj"]
ignore = ["**/obj/**", "**/bin/**"]

[ui]
vim_keys = true
theme = "default"          # or "modern"
show_duration = true
collapse_passed = false

[runner]
extra_args = ["--no-restore"]
default_framework = "all"  # or specific TFM like "net8.0"

[github]
# Optional: for private repos or higher rate limits
# Supports environment variable expansion
token = "${GITHUB_TOKEN}"

[bisect]
max_commits = 100          # Safety limit
build_timeout = 120
```

## Technical Design

### Dependencies

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
quick-xml = "0.37"        # Parse TRX results
notify = "7"
clap = { version = "4", features = ["derive"] }
directories = "5"
octocrab = "0.41"         # GitHub API
git2 = "0.19"             # Git operations
tree-sitter = "0.24"      # C# parsing for test detection
tree-sitter-c-sharp = "0.23"
```

### Project Structure

```
src/
├── main.rs
├── cli.rs
├── app.rs
├── config.rs
├── error.rs
├── ui/
│   ├── mod.rs
│   ├── layout.rs
│   ├── projects.rs
│   ├── tests.rs
│   ├── output.rs
│   ├── summary.rs
│   ├── pr_view.rs
│   ├── bisect_view.rs
│   └── theme.rs
├── runner/
│   ├── mod.rs
│   ├── discovery.rs
│   ├── executor.rs
│   └── watcher.rs
├── git/
│   ├── mod.rs
│   ├── pr.rs             # PR fetching and diff parsing
│   ├── bisect.rs         # Bisect logic
│   ├── credentials.rs    # Credential resolution
│   └── ops.rs            # Checkout, pull, stash, etc.
├── parser/
│   ├── mod.rs
│   └── csharp.rs         # Test detection in C# files
└── model/
    ├── mod.rs
    ├── project.rs
    ├── test.rs
    └── result.rs
```

### Data Model

```rust
pub enum TestStatus {
    Pending,
    Running,
    Passed { duration: Duration },
    Failed { 
        duration: Duration, 
        message: String,
        stack_trace: Option<String>,
    },
    Skipped { reason: Option<String> },
}

pub struct Test {
    pub id: String,
    pub name: String,
    pub fqn: String,
    pub status: TestStatus,
    pub output: Vec<String>,
}

pub struct TestClass {
    pub name: String,
    pub tests: Vec<Test>,
    pub collapsed: bool,
}

pub struct TestProject {
    pub name: String,
    pub path: PathBuf,
    pub classes: Vec<TestClass>,
    pub target_frameworks: Vec<String>,
    pub load_error: Option<String>,
}

pub struct AppState {
    pub projects: Vec<TestProject>,
    pub cursor: CursorPosition,           // Current cursor location
    pub selected: HashSet<TestId>,        // Multi-selected tests for batch operations
    pub filter: Option<String>,
    pub watch_mode: bool,
    pub pr_mode: Option<PrSession>,
    pub bisect_mode: Option<BisectSession>,
    pub output: OutputBuffer,
}

/// Identifies a specific test across the tree
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct TestId {
    pub project: String,
    pub class: String,
    pub test: String,
}

/// Where the cursor currently sits in the tree
pub struct CursorPosition {
    pub project_idx: usize,
    pub class_idx: Option<usize>,         // None = project row selected
    pub test_idx: Option<usize>,          // None = class row selected
}
```

### Keybindings

| Key                 | Action                     |
|---------------------|----------------------------|
| `↓`                 | Move down                  |
| `↑`                 | Move up                    |
| `Space`             | Toggle (collapse class or select test) |
| `b`                 | Build project only         |
| `r`                 | Run selected tests (or test at cursor if none selected) |
| `R`                 | Run all tests              |
| `a`                 | Run failed again           |
| `c`                 | Clear selection            |
| `w`                 | Toggle watch mode          |
| `x`                 | Clear output               |
| `P`                 | Run full project (PR mode) |
| `d`                 | View diff (PR mode)        |
| `s`                 | Skip commit (bisect mode)  |
| `/`                 | Filter                     |
| `Esc`               | Clear / cancel             |
| `Tab`               | Switch pane focus          |
| `y`                 | Copy output to clipboard   |
| `g`                 | Go to top                  |
| `G`                 | Go to bottom               |
| `q`                 | Quit                       |
| `?`                 | Help                       |

### Selection Model

Testament distinguishes between the **cursor** (where you are in the tree) and the **selection** (tests marked for batch operations):

- **Cursor**: Single position, moved with `j`/`k`/`h`/`l`. Highlighted with a background color.
- **Selection**: Zero or more tests marked with `Space`. Shown with a `*` marker.

When running tests:
- If any tests are selected (`selected.len() > 0`), `r` runs only those tests
- If nothing is selected, `r` runs whatever is under the cursor (single test, class, or project)
- `R` always runs all tests, ignoring selection
- `c` clears the selection

Selecting a class or project with `Space` selects all tests within it. This enables workflows like "run these 3 specific tests" or "run this whole class plus one test from another class."

### Event Loop

```rust
pub enum Event {
    Key(KeyEvent),
    Tick,
    TestsDiscovered(Vec<TestProject>),
    TestStarted { project: String, test: String },
    TestOutput { project: String, test: String, line: OutputLine },
    TestCompleted { project: String, test: String, result: TestStatus },
    RunCompleted { summary: RunSummary },
    FileChanged(PathBuf),
    PrFetched(PrInfo),
    PrTestsDetected(Vec<PrTestChange>),
    BisectStep(BisectStep),
    BisectComplete(CommitInfo),
    GitOperation { status: String },
    Error(TestamentError),
}
```

### dotnet Integration

**Discovery:**

```bash
dotnet test --list-tests --verbosity quiet
```

**Execution:**

```bash
dotnet test --filter "FullyQualifiedName~TestName" \
            --logger "trx;LogFileName=results.trx" \
            --verbosity normal
```

Parse the TRX (XML) output for structured results.

## Output Formats

### JSON (for CI integration)

```json
{
  "summary": {
    "passed": 44,
    "failed": 2,
    "skipped": 1,
    "duration_ms": 3247
  },
  "failed": [
    {
      "project": "Api.Tests",
      "class": "AuthTests",
      "test": "LogoutTest",
      "message": "Assert.Equal failed",
      "duration_ms": 127
    }
  ]
}
```
