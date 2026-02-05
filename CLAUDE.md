# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Testament is a Rust TUI application for discovering, running, and monitoring .NET tests. It wraps `dotnet test` with a terminal interface featuring real-time output streaming, watch mode, PR-based test running, and git bisect for finding failing commits.

**Status**: Phase 2 (Core Features) is mostly complete. The TUI can discover tests, display them in a four-pane layout with test class grouping, run them, and show detailed results for selected tests.

## Build and Run Commands

```bash
cargo build                    # Build the project
cargo run                      # Run the TUI
cargo run -- --help            # Show CLI help
cargo test                     # Run Rust tests
cargo clippy                   # Lint
cargo fmt                      # Format code
```

## Architecture (from SPEC.md)

The planned project structure:

```
src/
├── main.rs              # Entry point
├── cli.rs               # Clap-based CLI parsing
├── app.rs               # Main application state and event loop
├── config.rs            # .testament.toml parsing
├── error.rs             # TestamentError enum
├── ui/                  # Ratatui-based TUI components
│   ├── layout.rs        # Four-pane layout (projects/tests/output/test_result)
│   ├── projects.rs      # Project list widget
│   ├── tests.rs         # Test tree widget with collapsible classes
│   ├── output.rs        # Output pane widget
│   ├── test_result.rs   # Test result details widget
│   ├── pr_view.rs       # PR mode UI
│   ├── bisect_view.rs   # Bisect mode UI
│   └── theme.rs         # Color themes (default amber/gold, modern)
├── runner/              # Test execution
│   ├── discovery.rs     # Solution/csproj parsing, test detection
│   ├── executor.rs      # dotnet test invocation, TRX parsing
│   └── watcher.rs       # File watch with notify crate
├── git/                 # Git operations
│   ├── pr.rs            # GitHub PR fetching via octocrab
│   ├── bisect.rs        # Binary search for failing commit
│   ├── credentials.rs   # GITHUB_TOKEN, gh CLI, git credential helper
│   └── ops.rs           # Checkout, pull, stash operations via git2
├── parser/
│   └── csharp.rs        # Tree-sitter C# parsing for test detection
└── model/               # Data structures
    ├── project.rs       # TestProject, TestClass
    ├── test.rs          # Test, TestStatus
    └── result.rs        # RunSummary, output types
```

## Key Dependencies

- **ratatui/crossterm**: Terminal UI
- **tokio**: Async runtime for concurrent test execution
- **quick-xml**: Parse TRX test result files
- **octocrab**: GitHub API for PR mode
- **git2**: Git operations for bisect/PR checkout
- **tree-sitter-c-sharp**: Parse C# files to detect test methods
- **notify**: File system watching
- **clap**: CLI argument parsing

## Design Philosophy

**Prefer simplicity and performance over features.**

- Keep the code simple and direct. Avoid abstractions until they're clearly needed.
- Optimize for startup time and responsiveness. The TUI should feel instant.
- Don't add features not in SPEC.md. If something isn't specified, leave it out.
- Avoid over-engineering: no excessive error handling for impossible cases, no premature optimization, no "just in case" flexibility.
- When in doubt, write less code. A 50-line solution is better than a 200-line "robust" one.
- Shell out to `dotnet` and `git` CLIs where practical rather than reimplementing their logic.
- Don't use `#[allow(dead_code)]` to silence warnings. Remove unused code instead. Use `#[cfg(test)]` for code only needed in tests.

## File Size Guidelines

**General guidance: 200-500 lines per file.** This keeps files small enough to reason about while avoiding excessive fragmentation.

| File Type | Sweet Spot | Upper Limit |
|-----------|------------|-------------|
| mod.rs | 50-150 | Re-exports and glue only |
| Feature modules | 200-400 | 600 |
| Data models | 100-300 | 400 |
| UI components | 150-350 | 500 |
| Tests | 200-500 | Can be longer, grouped by fixture |

**When to split:**
- Multiple distinct responsibilities in one file
- You're scrolling a lot to find things
- The file has natural seams (e.g., `executor.rs` could become `executor/run.rs`, `executor/parse.rs`)
- More than 3-4 major structs/enums with their own impl blocks

**When longer is fine:**
- Single cohesive responsibility (e.g., a parser for one format)
- Splitting would create artificial boundaries
- Test files covering one module thoroughly

For Testament specifically, most files should land in the 150-400 range, with `app.rs` (state machine) and `executor.rs` (dotnet integration) potentially pushing toward 500-600 if they stay unified.

## Development Workflow

Work is done in feature branches and merged via PR to `main` at the end of each main phase.

**Versioning**: Bump the version in `Cargo.toml` for any functional change (new features, bug fixes, behavior changes). Follow semantic versioning: patch for fixes, minor for new features, major for breaking changes.

**Phase 1: Minimal Working TUI** (Complete)
- Cargo.toml, main.rs, cli.rs, error.rs
- Test project discovery, model structs
- Basic TUI with three panes, navigation
- Test execution, TRX parsing, results display

**Phase 2: Core Features**
- Expand/collapse, multi-select, filter
- Watch mode with file system notifications
- .testament.toml configuration file
- Re-run failed tests

**Phase 3: Advanced Features**
- PR test runner
- Git bisect for failing tests
- JSON output, multi-target framework grouping

## Documentation

When making functional changes:
- Update SPEC.md if adding/changing features or keybindings
- Update README.md if changing user-facing behavior (keybindings, usage, installation) and when updating version
- Add entry to CHANGELOG.md following Keep a Changelog format

## Core Concepts

- **Test Discovery**: Searches for `.sln` files, parses for test projects, or falls back to globbing for `*Tests.csproj`
- **Framework Support**: xUnit, NUnit, MSTest - detected via csproj package references
- **Output Streaming**: Real-time test output via `dotnet test --verbosity minimal` (build noise filtered)
- **PR Mode**: `testament pr <url|number>` - fetches PR diff, identifies changed tests, runs only those
- **Bisect Mode**: `testament bisect <test>` - binary search through git history to find failing commit
- **Configuration**: `.testament.toml` for project settings, watch patterns, runner options
