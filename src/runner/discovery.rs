use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::error::{Result, TestamentError};
use crate::model::{Test, TestClass, TestProject};
use crate::parser::build_test_name_map;

/// Strip Windows UNC prefix (\\?\) from path - dotnet CLI doesn't handle it well
fn strip_unc_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\?\") {
        PathBuf::from(&s[4..])
    } else {
        path.to_path_buf()
    }
}

/// Events sent during background test discovery
pub enum DiscoveryEvent {
    /// Tests discovered for a project (project index, test classes)
    ProjectDiscovered(usize, Vec<TestClass>),
    /// Discovery failed for a project (project index, error message)
    ProjectError(usize, String),
    /// All discovery complete
    Complete,
}

/// Find a .sln or .csproj file in the given path.
///
/// If `start` is a file with .sln or .csproj extension, returns it directly.
/// If `start` is a directory, searches that directory for .sln files first.
/// If no .sln found, walks up parent directories looking for a .sln.
/// Falls back to .csproj files in the original directory.
pub fn find_solution(start: &Path) -> Result<PathBuf> {
    // Canonicalize the path to resolve ./ and normalize separators
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    // Strip Windows UNC prefix (\\?\) which dotnet doesn't handle well
    let start = strip_unc_prefix(&start);
    
    // If start is a file, check if it's a valid solution/project file
    if start.is_file() {
        if let Some(ext) = start.extension() {
            if ext == "sln" || ext == "csproj" {
                return Ok(start.to_path_buf());
            }
        }
        return Err(TestamentError::NoSolutionFound);
    }

    // If start is a directory, search for .sln files
    if start.is_dir() {
        // Search current directory and parent directories for .sln files,
        // stopping at the git repository root
        let mut search_dir = Some(start.as_path());
        while let Some(dir) = search_dir {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "sln") {
                        return Ok(path);
                    }
                }
            }
            // Stop at git repo root to avoid finding unrelated solutions
            if dir.join(".git").exists() {
                break;
            }
            search_dir = dir.parent();
        }

        // No .sln found - fall back to .csproj in original directory
        let entries = std::fs::read_dir(&start)
            .map_err(|e| TestamentError::FileRead {
                path: start.to_path_buf(),
                source: e,
            })?;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "csproj") {
                return Ok(path);
            }
        }
    }

    Err(TestamentError::NoSolutionFound)
}

/// Find all .csproj files in a directory, searching recursively into subdirectories.
/// Skips `bin`, `obj`, and hidden directories.
pub fn find_csproj_in_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let dir = strip_unc_prefix(&dir);

    let mut csproj_files = Vec::new();
    find_csproj_recursive(&dir, &mut csproj_files);

    if csproj_files.is_empty() {
        Err(TestamentError::NoSolutionFound)
    } else {
        Ok(csproj_files)
    }
}

fn find_csproj_recursive(dir: &Path, results: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name == "bin" || name == "obj" || name.starts_with('.') {
                    continue;
                }
            }
            find_csproj_recursive(&path, results);
        } else if path.extension().is_some_and(|ext| ext == "csproj") {
            results.push(path);
        }
    }
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
                // Normalize path separators to the platform's native separator
                let normalized_path = if cfg!(windows) {
                    rel_path.replace('/', "\\")
                } else {
                    rel_path.replace('\\', "/")
                };
                let full_path = sln_dir.join(normalized_path);
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
    let project_paths = if path.extension().is_some_and(|ext| ext == "csproj") {
        vec![path.to_path_buf()]
    } else {
        parse_solution(path)?
    };

    discover_projects_from_paths(project_paths)
}

/// Discover test projects from explicit csproj paths.
/// Used by PR mode to only load projects containing changed tests.
pub fn discover_projects_from_paths(project_paths: Vec<PathBuf>) -> Result<(Vec<TestProject>, mpsc::Receiver<DiscoveryEvent>)> {

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
                    let project_dir = path.parent().unwrap_or(Path::new("."));

                    let test_result = list_tests(&path);

                    match test_result {
                        Ok(test_names) => {
                            let classes = if are_fqn_names(&test_names) {
                                group_tests_by_fqn(test_names)
                            } else {
                                let name_map = build_test_name_map(project_dir);
                                group_tests_by_class(test_names, &name_map)
                            };
                            let _ = tx.send(DiscoveryEvent::ProjectDiscovered(idx, classes));
                        }
                        Err(e) => {
                            let error_msg = format!("{}", e);
                            let _ = tx.send(DiscoveryEvent::ProjectError(idx, error_msg));
                        }
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

/// Run `dotnet test --list-tests` to get test names, then attempt to resolve
/// fully-qualified names via `dotnet vstest /ListFullyQualifiedTests`.
/// First tries cache, then --no-build for speed.
fn list_tests(project_path: &Path) -> Result<Vec<String>> {
    // Try cache first
    if let Some(cached) = load_cache(project_path) {
        return Ok(cached);
    }
    
    let project_dir = project_path.parent().unwrap_or(Path::new("."));
    
    // First try without building (fast if already built)
    let output = Command::new("dotnet")
        .args(["test", "--list-tests", "--no-build"])
        .arg(project_path)
        .current_dir(project_dir)
        .output()
        .map_err(|e| TestamentError::DotnetExecution(format!("Failed to spawn: {}", e)))?;

    // Don't build on discovery - only use --no-build result
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Filter out build noise to show actual errors
        let filter_build_noise = |s: &str| -> String {
            s.lines()
                .filter(|line| {
                    let trimmed = line.trim();
                    !trimmed.is_empty()
                        && !trimmed.starts_with("Determining projects")
                        && !trimmed.starts_with("All projects are up-to-date")
                        && !trimmed.starts_with("Restored ")
                        && !trimmed.contains("-> ") // Build output like "Project -> path.dll"
                        && !trimmed.starts_with("Build started")
                        && !trimmed.starts_with("Build succeeded")
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        
        let filtered_stdout = filter_build_noise(&stdout);
        let filtered_stderr = filter_build_noise(&stderr);
        
        let error_detail = if !filtered_stderr.is_empty() {
            filtered_stderr
        } else if !filtered_stdout.is_empty() {
            filtered_stdout
        } else {
            format!("Exit code: {:?}", output.status.code())
        };
        
        return Err(TestamentError::DotnetExecution(error_detail));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut tests = Vec::new();
    let mut in_test_list = false;
    let mut dll_path = None;

    for line in stdout.lines() {
        let trimmed = line.trim();
        // Extract DLL path from "Test run for <path> (<framework>)"
        if trimmed.starts_with("Test run for ") {
            if let Some(path_end) = trimmed.rfind(" (") {
                let path_str = &trimmed[13..path_end];
                let p = PathBuf::from(path_str);
                if p.exists() {
                    dll_path = Some(p);
                }
            }
        }
        if trimmed == "The following Tests are available:" {
            in_test_list = true;
            continue;
        }
        if in_test_list && !trimmed.is_empty() {
            tests.push(trimmed.to_string());
        }
    }

    // Try to get fully-qualified names via vstest
    if let Some(dll) = dll_path {
        if let Some(fqn_tests) = list_tests_fqn(&dll) {
            if fqn_tests.len() == tests.len() {
                save_cache(project_path, &fqn_tests);
                return Ok(fqn_tests);
            }
        }
    }

    // Save to cache for next time
    save_cache(project_path, &tests);
    
    Ok(tests)
}

/// Try to get fully-qualified test names using `dotnet vstest /ListFullyQualifiedTests`.
/// Returns None if the command fails or produces no output.
fn list_tests_fqn(dll_path: &Path) -> Option<Vec<String>> {
    // Use a unique temp file per DLL to avoid race conditions in parallel discovery
    let mut hasher = DefaultHasher::new();
    dll_path.hash(&mut hasher);
    let temp_file = std::env::temp_dir().join(format!("testament_fqn_{:x}.txt", hasher.finish()));
    let output = Command::new("dotnet")
        .arg("vstest")
        .arg(dll_path)
        .arg("/ListFullyQualifiedTests")
        .arg(format!("/ListTestsTargetPath:{}", temp_file.display()))
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let content = std::fs::read_to_string(&temp_file).ok()?;
    let _ = std::fs::remove_file(&temp_file);

    let tests: Vec<String> = content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    if tests.is_empty() {
        return None;
    }

    Some(tests)
}

/// Get cache file path for a project
fn get_cache_path(project_path: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    project_path.hash(&mut hasher);
    let hash = hasher.finish();
    std::env::temp_dir().join(format!("testament_discovery_{:x}.cache", hash))
}

/// Get the modification time of a project in millis (max of csproj mtime and newest DLL in bin/)
fn get_project_mtime(project_path: &Path) -> Option<u128> {
    let csproj_mtime = std::fs::metadata(project_path)
        .and_then(|m| m.modified())
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();

    let mut max_mtime = csproj_mtime;

    // Check newest DLL in bin/ - rebuilds update DLLs even when csproj doesn't change
    if let Some(project_dir) = project_path.parent() {
        let bin_dir = project_dir.join("bin");
        if let Ok(dll_mtime) = newest_file_mtime(&bin_dir, "dll") {
            max_mtime = max_mtime.max(dll_mtime);
        }
    }

    Some(max_mtime)
}

/// Find the newest file with the given extension under a directory (recursive).
fn newest_file_mtime(dir: &Path, ext: &str) -> std::result::Result<u128, std::io::Error> {
    let mut max: u128 = 0;
    newest_file_mtime_recursive(dir, ext, &mut max)?;
    Ok(max)
}

fn newest_file_mtime_recursive(dir: &Path, ext: &str, max: &mut u128) -> std::result::Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_dir() {
            let _ = newest_file_mtime_recursive(&entry.path(), ext, max);
        } else if ft.is_file() && entry.path().extension().is_some_and(|e| e == ext) {
            if let Ok(mtime) = entry.metadata()
                .and_then(|m| m.modified())
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis())
            {
                *max = (*max).max(mtime);
            }
        }
    }
    Ok(())
}

/// Try to load cached test list
fn load_cache(project_path: &Path) -> Option<Vec<String>> {
    let cache_path = get_cache_path(project_path);
    let content = std::fs::read_to_string(&cache_path).ok()?;
    let mut lines = content.lines();
    
    // First line is the mtime
    let cached_mtime: u128 = lines.next()?.parse().ok()?;
    let current_mtime = get_project_mtime(project_path)?;
    
    if cached_mtime != current_mtime {
        return None; // Cache is stale
    }
    
    Some(lines.map(|s| s.to_string()).collect())
}

/// Save test list to cache
fn save_cache(project_path: &Path, tests: &[String]) {
    let Some(mtime) = get_project_mtime(project_path) else { return };
    let cache_path = get_cache_path(project_path);
    
    let content = std::iter::once(mtime.to_string())
        .chain(tests.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n");
    
    let _ = std::fs::write(cache_path, content);
}

/// Resolve a test name against the name map using cascading lookup strategies.
/// Returns the matching infos and the lookup key used (for used_counts cycling).
fn resolve_test_in_map<'a>(
    method_name: &str,
    name_map: &'a std::collections::HashMap<String, Vec<crate::parser::TestMethodInfo>>,
) -> Option<(&'a Vec<crate::parser::TestMethodInfo>, String)> {
    // 1. Exact match (handles non-parameterized FQN or bare method name)
    if let Some(infos) = name_map.get(method_name) {
        return Some((infos, method_name.to_string()));
    }

    // 2. Strip parameterized test arguments: "NS.Class.Method(x: 1)" -> "NS.Class.Method"
    let base = method_name.split('(').next().unwrap_or(method_name);
    if base.len() < method_name.len() {
        if let Some(infos) = name_map.get(base) {
            return Some((infos, base.to_string()));
        }
    }

    // 3. Extract bare method name: "NS.Class.Method" or "NS.Class.Method(x: 1)" -> "Method"
    if let Some(pos) = base.rfind('.') {
        let bare = &base[pos + 1..];
        if let Some(infos) = name_map.get(bare) {
            return Some((infos, bare.to_string()));
        }
    }

    None
}

/// Check if test names are fully-qualified (e.g., "Namespace.Class.Method").
/// Returns true if most names contain 2+ dots (namespace.class.method).
fn are_fqn_names(test_names: &[String]) -> bool {
    if test_names.is_empty() {
        return false;
    }
    let fqn_count = test_names.iter()
        .filter(|n| {
            let base = n.split('(').next().unwrap_or(n);
            base.matches('.').count() >= 2
        })
        .count();
    fqn_count > test_names.len() / 2
}

/// Parse a fully-qualified test name into (method, class, namespace).
/// Input: "Namespace.SubNS.ClassName.MethodName" or "NS.Class.Method(args)"
/// Returns: (method_name_with_args, class_name, namespace)
fn parse_fqn(fqn: &str) -> (String, String, String) {
    let base = fqn.split('(').next().unwrap_or(fqn);
    let parts: Vec<&str> = base.rsplitn(3, '.').collect();
    match parts.len() {
        3 => (parts[0].to_string(), parts[1].to_string(), parts[2].to_string()),
        2 => (parts[0].to_string(), parts[1].to_string(), String::new()),
        _ => (fqn.to_string(), String::new(), String::new()),
    }
}

/// Group test names by their class using C# source parsing info.
/// Supports both fully-qualified names (from vstest) and bare method names (from dotnet test).
fn group_tests_by_class(
    test_names: Vec<String>,
    name_map: &std::collections::HashMap<String, Vec<crate::parser::TestMethodInfo>>,
) -> Vec<TestClass> {
    use std::collections::HashMap;

    // If names are FQN, parse directly without needing the name map
    if are_fqn_names(&test_names) {
        return group_tests_by_fqn(test_names);
    }

    let mut classes: HashMap<String, Vec<Test>> = HashMap::new();
    // Track usage index per key to cycle through entries for duplicate method names
    let mut used_counts: HashMap<String, usize> = HashMap::new();

    for method_name in test_names {
        // Look up the method in our parsed C# info with cascading fallbacks
        let (full_name, class_full_name) = if let Some((infos, lookup_key)) =
            resolve_test_in_map(&method_name, name_map)
        {
            let idx = used_counts.entry(lookup_key).or_insert(0);
            let info = &infos[(*idx) % infos.len()];
            *idx += 1;

            let full = info.full_name();
            let class_full = if info.namespace.is_empty() {
                info.class_name.clone()
            } else {
                format!("{}.{}", info.namespace, info.class_name)
            };
            (full, class_full)
        } else {
            // Fallback: no class info available, use method name directly
            (method_name.clone(), String::new())
        };

        let test = Test::new(method_name, full_name);
        classes.entry(class_full_name).or_default().push(test);
    }

    build_test_classes(classes)
}

/// Group tests using fully-qualified names parsed directly from the FQN string.
fn group_tests_by_fqn(test_names: Vec<String>) -> Vec<TestClass> {
    use std::collections::HashMap;

    let mut classes: HashMap<String, Vec<Test>> = HashMap::new();

    for fqn in test_names {
        let (method, class, namespace) = parse_fqn(&fqn);
        let full_name = if namespace.is_empty() {
            if class.is_empty() {
                method.clone()
            } else {
                format!("{}.{}", class, method)
            }
        } else {
            format!("{}.{}.{}", namespace, class, method)
        };
        let class_full = if namespace.is_empty() {
            class.clone()
        } else {
            format!("{}.{}", namespace, class)
        };
        // Display name: Class.Method (not the full namespace path)
        let display_name = if class.is_empty() {
            method
        } else {
            format!("{}.{}", class, method)
        };
        let test = Test::new(display_name, full_name);
        classes.entry(class_full).or_default().push(test);
    }

    build_test_classes(classes)
}

/// Convert a HashMap of class -> tests into sorted Vec<TestClass>.
fn build_test_classes(classes: std::collections::HashMap<String, Vec<Test>>) -> Vec<TestClass> {
    let mut result: Vec<TestClass> = classes
        .into_iter()
        .map(|(full_name, mut tests)| {
            let parts: Vec<&str> = full_name.rsplitn(2, '.').collect();
            let (class_name, namespace) = if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (full_name.clone(), String::new())
            };
            // Pre-sort tests by name_lower so render doesn't need to sort
            tests.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
            let mut class = TestClass::new(class_name, namespace);
            class.tests = tests;
            class
        })
        .collect();

    // Pre-sort classes by full_name_lower so render doesn't need to sort
    result.sort_by(|a, b| a.full_name_lower.cmp(&b.full_name_lower));
    result
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
    use crate::parser::TestMethodInfo;
    use std::collections::HashMap;

    fn make_test_info(method: &str, class: &str, namespace: &str) -> TestMethodInfo {
        TestMethodInfo {
            method_name: method.to_string(),
            class_name: class.to_string(),
            namespace: namespace.to_string(),
        }
    }

    #[test]
    fn test_group_tests_empty_list() {
        let map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();
        let result = group_tests_by_class(vec![], &map);
        assert!(result.is_empty());
    }

    #[test]
    fn test_group_tests_single_test_with_map() {
        let mut map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();
        map.entry("TestMethod".to_string())
            .or_default()
            .push(make_test_info("TestMethod", "MyClass", "MyNamespace"));

        let tests = vec!["TestMethod".to_string()];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "MyClass");
        assert_eq!(class.namespace, "MyNamespace");
        assert_eq!(class.tests.len(), 1);
        assert_eq!(class.tests[0].name, "TestMethod");
        assert_eq!(class.tests[0].full_name, "MyNamespace.MyClass.TestMethod");
    }

    #[test]
    fn test_group_tests_multiple_tests_same_class_with_map() {
        let mut map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();
        map.entry("Test1".to_string()).or_default().push(make_test_info("Test1", "MyClass", "NS"));
        map.entry("Test2".to_string()).or_default().push(make_test_info("Test2", "MyClass", "NS"));
        map.entry("Test3".to_string()).or_default().push(make_test_info("Test3", "MyClass", "NS"));

        let tests = vec!["Test1".to_string(), "Test2".to_string(), "Test3".to_string()];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "MyClass");
        assert_eq!(class.namespace, "NS");
        assert_eq!(class.tests.len(), 3);
    }

    #[test]
    fn test_group_tests_multiple_classes_with_map() {
        let mut map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();
        map.entry("Test1".to_string()).or_default().push(make_test_info("Test1", "ClassA", "NS"));
        map.entry("Test2".to_string()).or_default().push(make_test_info("Test2", "ClassB", "NS"));
        map.entry("Test3".to_string()).or_default().push(make_test_info("Test3", "ClassA", "NS"));

        let tests = vec!["Test1".to_string(), "Test2".to_string(), "Test3".to_string()];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 2);

        let class_a = result.iter().find(|c| c.name == "ClassA").unwrap();
        assert_eq!(class_a.tests.len(), 2);

        let class_b = result.iter().find(|c| c.name == "ClassB").unwrap();
        assert_eq!(class_b.tests.len(), 1);
    }

    #[test]
    fn test_group_tests_fallback_when_not_in_map() {
        let map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();

        let tests = vec!["TestMethod".to_string()];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        // Fallback: no class info, method goes into unnamed class
        assert_eq!(class.name, "");
        assert_eq!(class.namespace, "");
        assert_eq!(class.tests.len(), 1);
        assert_eq!(class.tests[0].name, "TestMethod");
    }

    #[test]
    fn test_group_tests_mixed_found_and_not_found() {
        let mut map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();
        map.entry("Test1".to_string())
            .or_default()
            .push(make_test_info("Test1", "MyClass", "NS"));

        let tests = vec!["Test1".to_string(), "UnknownTest".to_string()];
        let result = group_tests_by_class(tests, &map);

        // Test1 goes to NS.MyClass, UnknownTest goes to unnamed class
        assert_eq!(result.len(), 2);

        let known_class = result.iter().find(|c| c.name == "MyClass").unwrap();
        assert_eq!(known_class.tests.len(), 1);
        assert_eq!(known_class.tests[0].name, "Test1");

        let unknown_class = result.iter().find(|c| c.name.is_empty()).unwrap();
        assert_eq!(unknown_class.tests.len(), 1);
        assert_eq!(unknown_class.tests[0].name, "UnknownTest");
    }

    #[test]
    fn test_group_tests_same_method_name_different_classes() {
        // Simulates the bug: two classes with identically-named methods
        let mut map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();
        map.entry("ShouldInit".to_string()).or_default().push(make_test_info("ShouldInit", "ClassA", "NS"));
        map.entry("ShouldInit".to_string()).or_default().push(make_test_info("ShouldInit", "ClassB", "NS"));
        map.entry("ShouldSave".to_string()).or_default().push(make_test_info("ShouldSave", "ClassA", "NS"));
        map.entry("ShouldSave".to_string()).or_default().push(make_test_info("ShouldSave", "ClassB", "NS"));

        // dotnet test --list-tests returns each method name twice (once per class)
        let tests = vec![
            "ShouldInit".to_string(),
            "ShouldInit".to_string(),
            "ShouldSave".to_string(),
            "ShouldSave".to_string(),
        ];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 2);

        let class_a = result.iter().find(|c| c.name == "ClassA").unwrap();
        assert_eq!(class_a.tests.len(), 2);

        let class_b = result.iter().find(|c| c.name == "ClassB").unwrap();
        assert_eq!(class_b.tests.len(), 2);
    }

    #[test]
    fn test_group_tests_fqn_input_matches_full_name_key() {
        // dotnet test --list-tests outputs fully qualified names like "NS.MyClass.TestMethod"
        // The name map has both full_name and bare method_name keys
        let mut map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();
        let info = make_test_info("TestMethod", "MyClass", "MyNamespace");
        map.entry("MyNamespace.MyClass.TestMethod".to_string())
            .or_default()
            .push(info.clone());
        map.entry("TestMethod".to_string())
            .or_default()
            .push(info);

        let tests = vec!["MyNamespace.MyClass.TestMethod".to_string()];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "MyClass");
        assert_eq!(class.namespace, "MyNamespace");
        assert_eq!(class.tests.len(), 1);
    }

    #[test]
    fn test_group_tests_parameterized_test_names() {
        // dotnet test --list-tests outputs parameterized tests with arguments:
        // "NS.MyClass.TestMethod(x: 1, expected: true)"
        let mut map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();
        let info = make_test_info("TestMethod", "MyClass", "NS");
        map.entry("NS.MyClass.TestMethod".to_string())
            .or_default()
            .push(info.clone());
        map.entry("TestMethod".to_string())
            .or_default()
            .push(info);

        let tests = vec![
            "NS.MyClass.TestMethod(x: 1, expected: true)".to_string(),
            "NS.MyClass.TestMethod(x: 2, expected: false)".to_string(),
        ];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "MyClass");
        assert_eq!(class.namespace, "NS");
        assert_eq!(class.tests.len(), 2);
        // No Uncategorized class should exist
        assert!(result.iter().all(|c| !c.name.is_empty()));
    }

    #[test]
    fn test_group_tests_bare_method_name_fallback() {
        // Bare method name (no dots) matched via name_map when FQN isn't available
        let mut map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();
        let info = make_test_info("TestMethod", "MyClass", "MyNamespace");
        map.entry("TestMethod".to_string())
            .or_default()
            .push(info);

        let tests = vec!["TestMethod".to_string()];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "MyClass");
        assert_eq!(class.namespace, "MyNamespace");
    }

    #[test]
    fn test_group_tests_parameterized_bare_name_fallback() {
        // Parameterized bare name stripped and matched via name_map
        let mut map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();
        let info = make_test_info("Calculate", "CalcTests", "App");
        map.entry("Calculate".to_string())
            .or_default()
            .push(info);

        let tests = vec![
            "Calculate(a: 1, b: 2)".to_string(),
        ];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "CalcTests");
        assert_eq!(class.namespace, "App");
    }

    #[test]
    fn test_group_tests_fqn_direct_parsing() {
        // FQN names (from vstest) are parsed directly without name_map
        let map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();

        let tests = vec![
            "MyNamespace.MyClass.TestAdd".to_string(),
            "MyNamespace.MyClass.TestDelete".to_string(),
            "MyNamespace.OtherClass.TestSave".to_string(),
        ];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 2);
        let my_class = result.iter().find(|c| c.name == "MyClass").unwrap();
        assert_eq!(my_class.namespace, "MyNamespace");
        assert_eq!(my_class.tests.len(), 2);
        // Test names should be Class.Method, not full FQN
        assert_eq!(my_class.tests[0].name, "MyClass.TestAdd");
        assert_eq!(my_class.tests[0].full_name, "MyNamespace.MyClass.TestAdd");

        let other_class = result.iter().find(|c| c.name == "OtherClass").unwrap();
        assert_eq!(other_class.namespace, "MyNamespace");
        assert_eq!(other_class.tests.len(), 1);
        assert_eq!(other_class.tests[0].name, "OtherClass.TestSave");
    }

    #[test]
    fn test_group_tests_fqn_with_deep_namespace() {
        // FQN with deep namespace like Enterprise.Module.Business.Test.ClassName.Method
        let map: HashMap<String, Vec<TestMethodInfo>> = HashMap::new();

        let tests = vec![
            "Enterprise.Module.Business.Test.MyTest.TestAdd".to_string(),
            "Enterprise.Module.Business.Test.MyTest.TestDelete".to_string(),
        ];
        let result = group_tests_by_class(tests, &map);

        assert_eq!(result.len(), 1);
        let class = &result[0];
        assert_eq!(class.name, "MyTest");
        assert_eq!(class.namespace, "Enterprise.Module.Business.Test");
        assert_eq!(class.tests.len(), 2);
        // Display name is Class.Method, not full namespace path
        assert_eq!(class.tests[0].name, "MyTest.TestAdd");
    }

    // find_solution tests
    #[test]
    fn test_find_solution_in_current_directory() {
        let temp_dir = TempDir::new().unwrap();
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, "").unwrap();

        let result = find_solution(temp_dir.path()).unwrap();
        // Compare file names since canonicalize may add UNC prefix on Windows
        assert_eq!(result.file_name(), sln_path.file_name());
        assert!(result.exists());
    }

    #[test]
    fn test_find_solution_in_parent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let sln_path = temp_dir.path().join("Test.sln");
        fs::write(&sln_path, "").unwrap();

        // Create a subdirectory with a .csproj but no .sln
        let sub_dir = temp_dir.path().join("SubProject");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("SubProject.csproj"), "").unwrap();

        // Searching from the subdirectory should find the .sln in the parent
        let result = find_solution(&sub_dir).unwrap();
        assert_eq!(result.file_name(), sln_path.file_name());
    }

    #[test]
    fn test_find_solution_not_found() {
        let temp_dir = TempDir::new().unwrap();
        // Create .git to stop parent traversal
        fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
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

    // find_csproj_in_dir tests
    #[test]
    fn test_find_csproj_in_dir_single_file() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("MyProject.csproj"), "").unwrap();

        let result = find_csproj_in_dir(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].extension().unwrap() == "csproj");
    }

    #[test]
    fn test_find_csproj_in_dir_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("ProjectA.csproj"), "").unwrap();
        fs::write(temp_dir.path().join("ProjectB.csproj"), "").unwrap();
        fs::write(temp_dir.path().join("ProjectC.csproj"), "").unwrap();

        let result = find_csproj_in_dir(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_find_csproj_in_dir_ignores_non_csproj() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("MyProject.csproj"), "").unwrap();
        fs::write(temp_dir.path().join("readme.md"), "").unwrap();
        fs::write(temp_dir.path().join("MyProject.fsproj"), "").unwrap();

        let result = find_csproj_in_dir(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_find_csproj_in_dir_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = find_csproj_in_dir(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_find_csproj_in_dir_no_csproj_files() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("readme.md"), "").unwrap();
        fs::write(temp_dir.path().join("Test.sln"), "").unwrap();

        let result = find_csproj_in_dir(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_find_csproj_in_dir_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let sub_a = temp_dir.path().join("ProjectA");
        let sub_b = temp_dir.path().join("ProjectB");
        fs::create_dir_all(&sub_a).unwrap();
        fs::create_dir_all(&sub_b).unwrap();
        fs::write(sub_a.join("ProjectA.csproj"), "").unwrap();
        fs::write(sub_b.join("ProjectB.csproj"), "").unwrap();

        let result = find_csproj_in_dir(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_find_csproj_in_dir_skips_bin_obj() {
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join("src");
        let bin = temp_dir.path().join("bin");
        let obj = temp_dir.path().join("obj");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&obj).unwrap();
        fs::write(src.join("Real.csproj"), "").unwrap();
        fs::write(bin.join("Cached.csproj"), "").unwrap();
        fs::write(obj.join("Generated.csproj"), "").unwrap();

        let result = find_csproj_in_dir(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].to_string_lossy().contains("Real"));
    }

    #[test]
    fn test_find_csproj_in_dir_mixed_levels() {
        let temp_dir = TempDir::new().unwrap();
        let sub = temp_dir.path().join("SubDir");
        fs::create_dir_all(&sub).unwrap();
        fs::write(temp_dir.path().join("Root.csproj"), "").unwrap();
        fs::write(sub.join("Nested.csproj"), "").unwrap();

        let result = find_csproj_in_dir(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 2);
    }
}
