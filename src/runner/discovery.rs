use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Result, TestamentError};
use crate::model::{Test, TestClass, TestProject};

/// Find a .sln or .csproj file in the given path.
///
/// If `start` is a file with .sln or .csproj extension, returns it directly.
/// If `start` is a directory, searches only that directory (non-recursively) for .sln files first,
/// then .csproj files.
pub fn find_solution(start: &Path) -> Result<PathBuf> {
    // If start is a file, check if it's a valid solution/project file
    if start.is_file() {
        if let Some(ext) = start.extension() {
            if ext == "sln" || ext == "csproj" {
                return Ok(start.to_path_buf());
            }
        }
        return Err(TestamentError::NoSolutionFound);
    }

    // If start is a directory, search only in that directory (non-recursively)
    if start.is_dir() {
        let entries: Vec<_> = std::fs::read_dir(start)
            .map_err(|e| TestamentError::FileRead {
                path: start.to_path_buf(),
                source: e,
            })?
            .filter_map(|e| e.ok())
            .collect();

        // First, look for .sln files
        for entry in &entries {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "sln") {
                return Ok(path);
            }
        }

        // If no .sln found, look for .csproj files
        for entry in &entries {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "csproj") {
                return Ok(path);
            }
        }
    }

    Err(TestamentError::NoSolutionFound)
}

/// Parse a .sln file to extract test project paths.
/// Looks for projects ending in Tests or Test.
pub fn parse_solution(sln_path: &Path) -> Result<Vec<PathBuf>> {
    let content = std::fs::read_to_string(sln_path).map_err(|e| TestamentError::FileRead {
        path: sln_path.to_path_buf(),
        source: e,
    })?;

    let sln_dir = sln_path.parent().unwrap_or(Path::new("."));
    let mut projects = Vec::new();

    // Simple regex-like parsing for Project lines
    // Format: Project("{GUID}") = "Name", "Path.csproj", "{GUID}"
    for line in content.lines() {
        if !line.starts_with("Project(") {
            continue;
        }
        // Extract the path between the second pair of quotes
        let parts: Vec<&str> = line.split('"').collect();
        if parts.len() >= 6 {
            let name = parts[3];
            let rel_path = parts[5];

            // Only include test projects
            if is_test_project_name(name) && rel_path.ends_with(".csproj") {
                let full_path = sln_dir.join(rel_path.replace('\\', "/"));
                if full_path.exists() {
                    projects.push(full_path);
                }
            }
        }
    }

    Ok(projects)
}

fn is_test_project_name(name: &str) -> bool {
    name.ends_with("Tests") || name.ends_with("Test") || name.ends_with(".Tests") || name.ends_with(".Test")
}

/// Discover test projects and their tests.
///
/// If `path` is a .csproj file, treats it as the single project.
/// If `path` is a .sln file, parses it to find test projects.
pub fn discover_projects(path: &Path) -> Result<Vec<TestProject>> {
    let project_paths = if path.extension().map_or(false, |ext| ext == "csproj") {
        // Directly use the .csproj file
        vec![path.to_path_buf()]
    } else {
        // Parse the .sln file
        parse_solution(path)?
    };

    let mut projects = Vec::new();

    for path in project_paths {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let mut project = TestProject::new(name, path.clone());

        // Run dotnet test --list-tests to discover tests
        if let Ok(tests) = list_tests(&path) {
            project.classes = group_tests_by_class(tests);
        }

        projects.push(project);
    }

    Ok(projects)
}

/// Run `dotnet test --list-tests` to get test names.
fn list_tests(project_path: &Path) -> Result<Vec<String>> {
    let output = Command::new("dotnet")
        .args(["test", "--list-tests"])
        .arg(project_path)
        .output()
        .map_err(|e| TestamentError::DotnetExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(TestamentError::DotnetExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut tests = Vec::new();
    let mut in_test_list = false;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed == "The following Tests are available:" {
            in_test_list = true;
            continue;
        }
        if in_test_list && !trimmed.is_empty() {
            tests.push(trimmed.to_string());
        }
    }

    Ok(tests)
}

/// Group test names by their class.
fn group_tests_by_class(test_names: Vec<String>) -> Vec<TestClass> {
    use std::collections::HashMap;

    let mut classes: HashMap<String, Vec<Test>> = HashMap::new();

    for full_name in test_names {
        // Parse "Namespace.ClassName.MethodName" format
        let parts: Vec<&str> = full_name.rsplitn(2, '.').collect();
        let (method_name, class_full_name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            (full_name.as_str(), "")
        };

        let test = Test::new(method_name.to_string(), full_name.clone());
        classes
            .entry(class_full_name.to_string())
            .or_default()
            .push(test);
    }

    classes
        .into_iter()
        .map(|(full_name, tests)| {
            let parts: Vec<&str> = full_name.rsplitn(2, '.').collect();
            let (class_name, namespace) = if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (full_name.clone(), String::new())
            };
            TestClass {
                name: class_name,
                namespace,
                tests,
            }
        })
        .collect()
}
