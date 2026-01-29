# Testament

A terminal UI for discovering, running, and monitoring .NET tests.

Testament wraps `dotnet test` with an interactive interface featuring real-time output streaming, watch mode, and keyboard-driven navigation.

![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Test Discovery** - Automatically finds test projects in your solution
- **Three-Pane UI** - Projects, tests, and output in a single view
- **Real-time Output** - Stream test output as it runs
- **Watch Mode** - Auto-run tests when files change
- **Filter & Select** - Filter tests by name, select specific tests to run
- **Run Failed** - Re-run only the tests that failed

## Installation

### From Source

Requires [Rust](https://rustup.rs/) 1.70 or later.

```bash
git clone https://github.com/craigrusselltiu/testament.git
cd testament
cargo install --path .
```

### Pre-built Binaries

Coming soon.

## Usage

Navigate to a directory containing a .NET solution or test project and run:

```bash
testament
```

Testament will find your `.sln` file, discover test projects, and display them in the TUI.

### Keybindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `r` | Run tests (all or selected) |
| `a` | Run failed tests from last run |
| `w` | Toggle watch mode |
| `j` / `k` | Move down / up |
| `Tab` | Switch to next pane |
| `Shift+Tab` | Switch to previous pane |
| `h` / `l` | Collapse / expand test class |
| `Space` | Toggle test selection |
| `c` | Clear all selections |
| `/` | Start filter mode |
| `Esc` | Clear filter |

### Panes

1. **Projects** (left) - List of test projects in your solution
2. **Tests** (middle) - Test classes and methods for the selected project
3. **Output** (right) - Test execution output and results

### Watch Mode

Press `w` to enable watch mode. Testament will monitor `.cs` and `.csproj` files and automatically re-run tests when changes are detected.

### Running Specific Tests

1. Navigate to the Tests pane with `Tab`
2. Use `j`/`k` to navigate to a test
3. Press `Space` to select it (repeat for multiple tests)
4. Press `r` to run only selected tests
5. Press `c` to clear selection

### Filtering Tests

1. Press `/` to enter filter mode
2. Type your filter text (case-insensitive)
3. Press `Enter` to apply
4. Press `Esc` to clear the filter

## Configuration

Create a `.testament.toml` file in your solution directory:

```toml
[runner]
parallel = 4                    # Number of parallel test workers (0 = default)
extra_args = ["--no-build"]     # Additional args passed to dotnet test

[watch]
debounce_ms = 500               # Delay before running tests after file change
patterns = ["*.cs", "*.csproj"] # File patterns to watch
ignore = ["**/obj/**", "**/bin/**"]  # Patterns to ignore
```

## Requirements

- .NET SDK (for `dotnet test`)
- A .NET solution with test projects (xUnit, NUnit, or MSTest)

## How It Works

1. Testament searches upward from the current directory for a `.sln` file
2. Parses the solution to find test projects (projects ending in `Tests` or `Test`)
3. Runs `dotnet test --list-tests` to discover individual tests
4. Executes tests with `dotnet test --logger trx` and parses the TRX results
5. Displays results in real-time in the TUI

## License

MIT
