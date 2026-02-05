use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;

use crate::error::{Result, TestamentError};
use crate::model::{Test, TestClass, TestProject};

/// Events sent during background test discovery
pub enum DiscoveryEvent {
    /// Tests discovered for a project (project index, test classes)
    ProjectDiscovered(usize, Vec<TestClass>),
    /// All discovery complete
    Complete,
}

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

/// Discover test projects lazily - returns projects immediately (without tests),
/// then discovers tests in background and sends results via channel.
///
/// This allows the TUI to start instantly while test discovery happens in background.
pub fn discover_projects_lazy(path: &Path) -> Result<(Vec<TestProject>, mpsc::Receiver<DiscoveryEvent>)> {
    let project_paths = if path.extension().map_or(false, |ext| ext == "csproj") {
        vec![path.to_path_buf()]
    } else {
        parse_solution(path)?
    };

    // Create projects without tests (instant)
    let projects: Vec<TestProject> = project_paths
        .iter()
        .map(|path| {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();
            TestProject::new(name, path.clone())
        })
        .collect();

    let (tx, rx) = mpsc::channel();

    // Spawn background discovery
    let paths_with_indices: Vec<_> = project_paths.into_iter().enumerate().collect();
    std::thread::spawn(move || {
        // Discover tests in parallel
        let handles: Vec<_> = paths_with_indices
            .into_iter()
            .map(|(idx, path)| {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    if let Ok(tests) = list_tests(&path) {
                        let classes = group_tests_by_class(tests);
                        let _ = tx.send(DiscoveryEvent::ProjectDiscovered(idx, classes));
                    }
                })
            })
            .collect();

        // Wait for all to complete
        for handle in handles {
            let _ = handle.join();
        }
        let _ = tx.send(DiscoveryEvent::Complete);
    });

    Ok((projects, rx))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // is_test_project_name tests
    #[test]
    fn test_is_test_project_name_with_tests_suffix() {
        assert!(is_test_project_name("MyProjectTests"));
        assert!(is_test_project_name("UnitTests"));
        assert!(is_test_project_name("IntegrationTests"));
    }

    #[test]
    fn test_is_test_project_name_with_test_suffix() {
        assert!(is_test_project_name("MyProjectTest"));
        assert!(is_test_project_name("UnitTest"));
        assert!(is_test_project_name("IntegrationTest"));
    }

    #[test]
    fn test_is_test_project_name_with_dot_tests_suffix() {
        assert!(is_test_project_name("MyProject.Tests"));
        assert!(is_test_project_name("Company.Product.Tests"));
    }

    #[test]
    fn test_is_test_project_name_with_dot_test_suffix() {
        assert!(is_test_project_name("MyProject.Test"));
        assert!(is_test_project_name("Company.Product.Test"));
    }

    #[test]
    fn test_is_test_project_name_non_test_projects() {
        assert!(!is_test_project_name("MyProject"));
        assert!(!is_test_project_name("TestUtilities"));
        assert!(!is_test_project_name("Testing"));
        assert!(!is_test_project_name("TestsData"));
        assert!(!is_test_project_name("TestHelper"));
    }

    #[test]
    fn test_is_test_project_name_empty() {
        assert!(!is_test_project_name(""));
    }

    #[test]
    fn test_is_test_project_name_case_sensitive() {
        assert!(!is_test_project_name("MyProjecttests"));
        assert!(!is_test_project_name("MyProjectTESTS"));
        assert!(!is_test_project_name("MyProjecttest"));
    }

    // group_tests_by_class tests
    #[test]
    fn test_group_tests_empty_list() {
        let result = group_tests_by_class(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_group_tests_single_test() {
        let tests = vec!["MyNamespace.MyClass.TestMethod".to_string()];
        let result = group_tests_by_class(tests);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "MyClass");
        assert_eq!(class.namespace, "MyNamespace");
        assert_eq!(class.tests.len(), 1);
        assert_eq!(class.tests[0].name, "TestMethod");
        assert_eq!(class.tests[0].full_name, "MyNamespace.MyClass.TestMethod");
    }

    #[test]
    fn test_group_tests_multiple_tests_same_class() {
        let tests = vec![
            "NS.Class.Test1".to_string(),
            "NS.Class.Test2".to_string(),
            "NS.Class.Test3".to_string(),
        ];
        let result = group_tests_by_class(tests);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "Class");
        assert_eq!(class.namespace, "NS");
        assert_eq!(class.tests.len(), 3);
    }

    #[test]
    fn test_group_tests_multiple_classes() {
        let tests = vec![
            "NS.ClassA.Test1".to_string(),
            "NS.ClassB.Test1".to_string(),
            "NS.ClassA.Test2".to_string(),
        ];
        let result = group_tests_by_class(tests);

        assert_eq!(result.len(), 2);

        let class_a = result.iter().find(|c| c.name == "ClassA").unwrap();
        assert_eq!(class_a.tests.len(), 2);

        let class_b = result.iter().find(|c| c.name == "ClassB").unwrap();
        assert_eq!(class_b.tests.len(), 1);
    }

    #[test]
    fn test_group_tests_nested_namespace() {
        let tests = vec!["Company.Product.Feature.Tests.MyClass.TestMethod".to_string()];
        let result = group_tests_by_class(tests);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "MyClass");
        assert_eq!(class.namespace, "Company.Product.Feature.Tests");
    }

    #[test]
    fn test_group_tests_no_namespace() {
        let tests = vec!["MyClass.TestMethod".to_string()];
        let result = group_tests_by_class(tests);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "MyClass");
        assert_eq!(class.namespace, "");
    }

    #[test]
    fn test_group_tests_only_method_name() {
        let tests = vec!["TestMethod".to_string()];
        let result = group_tests_by_class(tests);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "");
        assert_eq!(class.namespace, "");
        assert_eq!(class.tests.len(), 1);
        assert_eq!(class.tests[0].name, "TestMethod");
    }

    #[test]
    fn test_group_tests_preserves_full_name() {
        let full_name = "Very.Long.Namespace.Path.ClassName.MethodName".to_string();
        let tests = vec![full_name.clone()];
        let result = group_tests_by_class(tests);

        assert_eq!(result[0].tests[0].full_name, full_name);
    }

    // find_solution tests
    #[test]
    fn test_find_solution_in_current_directory() {
        let temp_dir = TempDir::new().unwrap();
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, "").unwrap();

        let result = find_solution(temp_dir.path()).unwrap();
        assert_eq!(result, sln_path);
    }

    #[test]
    fn test_find_solution_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let result = find_solution(temp_dir.path());

        assert!(result.is_err());
        match result {
            Err(TestamentError::NoSolutionFound) => (),
            _ => panic!("Expected NoSolutionFound error"),
        }
    }

    #[test]
    fn test_find_solution_multiple_sln_files() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("First.sln"), "").unwrap();
        fs::write(temp_dir.path().join("Second.sln"), "").unwrap();

        let result = find_solution(temp_dir.path()).unwrap();
        assert!(result.extension().map_or(false, |ext| ext == "sln"));
    }

    // parse_solution tests
    #[test]
    fn test_parse_solution_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, "").unwrap();

        let result = parse_solution(&sln_path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_solution_no_test_projects() {
        let temp_dir = TempDir::new().unwrap();
        let sln_content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyProject", "MyProject\MyProject.csproj", "{12345678-1234-1234-1234-123456789012}"
EndProject
"#;
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, sln_content).unwrap();

        let result = parse_solution(&sln_path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_solution_with_test_project() {
        let temp_dir = TempDir::new().unwrap();

        // Create test project file
        let project_dir = temp_dir.path().join("MyProjectTests");
        fs::create_dir_all(&project_dir).unwrap();
        let csproj_path = project_dir.join("MyProjectTests.csproj");
        fs::write(&csproj_path, "<Project></Project>").unwrap();

        let sln_content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyProjectTests", "MyProjectTests\MyProjectTests.csproj", "{12345678-1234-1234-1234-123456789012}"
EndProject
"#;
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, sln_content).unwrap();

        let result = parse_solution(&sln_path).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("MyProjectTests.csproj"));
    }

    #[test]
    fn test_parse_solution_with_multiple_test_projects() {
        let temp_dir = TempDir::new().unwrap();

        // Create test project files
        for name in &["UnitTests", "IntegrationTests"] {
            let project_dir = temp_dir.path().join(name);
            fs::create_dir_all(&project_dir).unwrap();
            let csproj_path = project_dir.join(format!("{}.csproj", name));
            fs::write(&csproj_path, "<Project></Project>").unwrap();
        }

        let sln_content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "UnitTests", "UnitTests\UnitTests.csproj", "{11111111-1111-1111-1111-111111111111}"
EndProject
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "IntegrationTests", "IntegrationTests\IntegrationTests.csproj", "{22222222-2222-2222-2222-222222222222}"
EndProject
"#;
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, sln_content).unwrap();

        let result = parse_solution(&sln_path).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_solution_mixed_projects() {
        let temp_dir = TempDir::new().unwrap();

        // Create both regular and test project files
        for name in &["MyProject", "MyProjectTests"] {
            let project_dir = temp_dir.path().join(name);
            fs::create_dir_all(&project_dir).unwrap();
            let csproj_path = project_dir.join(format!("{}.csproj", name));
            fs::write(&csproj_path, "<Project></Project>").unwrap();
        }

        let sln_content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyProject", "MyProject\MyProject.csproj", "{11111111-1111-1111-1111-111111111111}"
EndProject
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyProjectTests", "MyProjectTests\MyProjectTests.csproj", "{22222222-2222-2222-2222-222222222222}"
EndProject
"#;
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, sln_content).unwrap();

        let result = parse_solution(&sln_path).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].to_string_lossy().contains("MyProjectTests"));
    }

    #[test]
    fn test_parse_solution_with_backslash_paths() {
        let temp_dir = TempDir::new().unwrap();

        let project_dir = temp_dir.path().join("src").join("MyProjectTests");
        fs::create_dir_all(&project_dir).unwrap();
        let csproj_path = project_dir.join("MyProjectTests.csproj");
        fs::write(&csproj_path, "<Project></Project>").unwrap();

        let sln_content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyProjectTests", "src\MyProjectTests\MyProjectTests.csproj", "{12345678-1234-1234-1234-123456789012}"
EndProject
"#;
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, sln_content).unwrap();

        let result = parse_solution(&sln_path).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_solution_missing_csproj_file() {
        let temp_dir = TempDir::new().unwrap();

        // Don't create the actual .csproj file
        let sln_content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyProjectTests", "MyProjectTests\MyProjectTests.csproj", "{12345678-1234-1234-1234-123456789012}"
EndProject
"#;
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, sln_content).unwrap();

        let result = parse_solution(&sln_path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_solution_non_csproj_extension() {
        let temp_dir = TempDir::new().unwrap();

        let project_dir = temp_dir.path().join("MyProjectTests");
        fs::create_dir_all(&project_dir).unwrap();
        let fsproj_path = project_dir.join("MyProjectTests.fsproj");
        fs::write(&fsproj_path, "<Project></Project>").unwrap();

        let sln_content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyProjectTests", "MyProjectTests\MyProjectTests.fsproj", "{12345678-1234-1234-1234-123456789012}"
EndProject
"#;
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, sln_content).unwrap();

        let result = parse_solution(&sln_path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_solution_file_not_found() {
        let result = parse_solution(Path::new("/nonexistent/path/Test.sln"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_solution_malformed_project_line() {
        let temp_dir = TempDir::new().unwrap();
        let sln_content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "Incomplete
"#;
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, sln_content).unwrap();

        let result = parse_solution(&sln_path).unwrap();
        assert!(result.is_empty());
    }
}
