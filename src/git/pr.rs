use std::process::Command;
use regex::Regex;

use crate::error::TestamentError;

/// Information extracted from a GitHub PR URL
#[derive(Debug, Clone)]
pub struct PrInfo {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

/// Parse a GitHub PR URL into its components
/// Supports: https://github.com/owner/repo/pull/123
pub fn parse_pr_url(url: &str) -> Result<PrInfo, TestamentError> {
    let re = Regex::new(r"https?://github\.com/([^/]+)/([^/]+)/pull/(\d+)")
        .map_err(|e| TestamentError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())))?;

    let caps = re.captures(url).ok_or_else(|| {
        TestamentError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Invalid PR URL format: {}", url),
        ))
    })?;

    let owner = caps.get(1).unwrap().as_str().to_string();
    let repo = caps.get(2).unwrap().as_str().to_string();
    let number: u64 = caps.get(3).unwrap().as_str().parse().map_err(|_| {
        TestamentError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid PR number",
        ))
    })?;

    Ok(PrInfo { owner, repo, number })
}

/// Get GitHub token from environment or gh CLI
pub fn get_github_token() -> Option<String> {
    // First try GITHUB_TOKEN environment variable
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }

    // Fall back to gh CLI
    let output = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;

    if output.status.success() {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            return Some(token);
        }
    }

    None
}

/// Fetch the diff for a PR from GitHub API
pub fn fetch_pr_diff(info: &PrInfo, token: Option<&str>) -> Result<String, TestamentError> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/pulls/{}", 
        info.owner, info.repo, info.number
    );

    let client = reqwest::blocking::Client::new();
    let mut request = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3.diff")
        .header("User-Agent", "testament");

    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().map_err(|e| {
        TestamentError::Io(std::io::Error::other(format!("Failed to fetch PR: {}", e)))
    })?;

    if !response.status().is_success() {
        return Err(TestamentError::Io(std::io::Error::other(format!(
            "GitHub API error: {} - {}",
            response.status(),
            response.text().unwrap_or_default()
        ))));
    }

    response.text().map_err(|e| {
        TestamentError::Io(std::io::Error::other(format!(
            "Failed to read response: {}",
            e
        )))
    })
}

/// Represents a changed test found in the PR diff
#[derive(Debug, Clone)]
pub struct ChangedTest {
    pub file_path: String,
    #[allow(dead_code)]
    pub class_name: String,
    pub method_name: String,
    #[allow(dead_code)]
    pub full_name: String,
}

/// Extract changed test methods from a unified diff
pub fn extract_changed_tests(diff: &str) -> Vec<ChangedTest> {
    let mut changed_tests = Vec::new();
    let mut current_file: Option<String> = None;
    let mut added_lines = String::new();
    let mut in_hunk = false;

    for line in diff.lines() {
        // Track which file we're in
        if let Some(file_path) = line.strip_prefix("+++ b/") {
            // Save tests from previous file
            if let Some(ref file) = current_file {
                if file.ends_with(".cs") && is_test_file(file) {
                    extract_tests_from_added_lines(file, &added_lines, &mut changed_tests);
                }
            }
            current_file = Some(file_path.to_string());
            added_lines.clear();
            in_hunk = false;
        } else if line.starts_with("@@") {
            in_hunk = true;
        } else if in_hunk && line.starts_with('+') && !line.starts_with("+++") {
            // This is an added line
            added_lines.push_str(&line[1..]);
            added_lines.push('\n');
        }
    }

    // Process the last file
    if let Some(ref file) = current_file {
        if file.ends_with(".cs") && is_test_file(file) {
            extract_tests_from_added_lines(file, &added_lines, &mut changed_tests);
        }
    }

    changed_tests
}

/// Check if a file path looks like a test file
fn is_test_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("test") || lower.contains("spec")
}

/// Extract test method names from added lines using regex pattern matching.
/// Since diff hunks may contain incomplete code, use pattern matching instead of tree-sitter.
fn extract_tests_from_added_lines(file_path: &str, added_content: &str, tests: &mut Vec<ChangedTest>) {
    // Pattern to match test attributes
    let test_attr_pattern = Regex::new(
        r"(?i)\[(Fact|Theory|Test|TestMethod|TestCase)\b[^\]]*\]"
    ).expect("Invalid regex");
    
    // Pattern to match method declarations (public void/async methods)
    let method_pattern = Regex::new(
        r"(?:public\s+)?(?:async\s+)?(?:Task|void)\s+(\w+)\s*\("
    ).expect("Invalid regex");
    
    // Pattern to match methods that look like tests by name (Test*, *Test, *Tests, Should*, etc.)
    let test_name_pattern = Regex::new(
        r"^(Test\w*|\w+Test|\w+Tests|\w+Should\w*|Should\w+)$"
    ).expect("Invalid regex");
    
    let lines: Vec<&str> = added_content.lines().collect();
    let mut found_methods = std::collections::HashSet::new();
    
    for i in 0..lines.len() {
        let line = lines[i].trim();
        
        // Strategy 1: Look for test attributes followed by method declarations
        if test_attr_pattern.is_match(line) {
            let end_idx = std::cmp::min(i + 5, lines.len());
            for next_line in lines.iter().take(end_idx).skip(i + 1) {
                let next_line = next_line.trim();
                if let Some(caps) = method_pattern.captures(next_line) {
                    if let Some(method_name) = caps.get(1) {
                        found_methods.insert(method_name.as_str().to_string());
                        break;
                    }
                }
            }
        }
        
        // Strategy 2: Look for method declarations that look like tests by naming convention
        if let Some(caps) = method_pattern.captures(line) {
            if let Some(method_name) = caps.get(1) {
                let name = method_name.as_str();
                if test_name_pattern.is_match(name) {
                    found_methods.insert(name.to_string());
                }
            }
        }
    }
    
    // Add all found methods as changed tests
    for method_name in found_methods {
        let (namespace, class_name) = extract_namespace_class_from_path(file_path);
        let full_name = if namespace.is_empty() {
            format!("{}.{}", class_name, method_name)
        } else {
            format!("{}.{}.{}", namespace, class_name, method_name)
        };
        
        tests.push(ChangedTest {
            file_path: file_path.to_string(),
            class_name,
            method_name,
            full_name,
        });
    }
}

/// Extract likely namespace and class name from file path
fn extract_namespace_class_from_path(path: &str) -> (String, String) {
    // Get the filename without extension as class name
    let file_name = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown");
    
    // Try to extract namespace from directory structure
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    let namespace = if parts.len() > 2 {
        // Skip the file and take parent directories that look like namespace parts
        parts[..parts.len() - 1]
            .iter()
            .filter(|p| !p.is_empty() && !p.contains('.'))
            .copied()
            .collect::<Vec<_>>()
            .join(".")
    } else {
        String::new()
    };
    
    (namespace, file_name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pr_url_valid() {
        let info = parse_pr_url("https://github.com/owner/repo/pull/123").unwrap();
        assert_eq!(info.owner, "owner");
        assert_eq!(info.repo, "repo");
        assert_eq!(info.number, 123);
    }

    #[test]
    fn test_parse_pr_url_with_https() {
        let info = parse_pr_url("https://github.com/microsoft/vscode/pull/9999").unwrap();
        assert_eq!(info.owner, "microsoft");
        assert_eq!(info.repo, "vscode");
        assert_eq!(info.number, 9999);
    }

    #[test]
    fn test_parse_pr_url_invalid() {
        assert!(parse_pr_url("https://gitlab.com/owner/repo/pull/123").is_err());
        assert!(parse_pr_url("not a url").is_err());
        assert!(parse_pr_url("https://github.com/owner/repo/issues/123").is_err());
    }

    #[test]
    fn test_is_test_file() {
        assert!(is_test_file("MyClassTests.cs"));
        assert!(is_test_file("src/Tests/MyTest.cs"));
        assert!(is_test_file("Api.Tests/UserSpec.cs"));
        assert!(!is_test_file("MyClass.cs"));
        assert!(!is_test_file("Program.cs"));
    }

    #[test]
    fn test_extract_changed_tests_simple_diff() {
        let diff = r#"diff --git a/Tests/MyTests.cs b/Tests/MyTests.cs
--- a/Tests/MyTests.cs
+++ b/Tests/MyTests.cs
@@ -10,6 +10,12 @@ public class MyTests
+    [Fact]
+    public void NewTest()
+    {
+        Assert.True(true);
+    }
"#;
        let tests = extract_changed_tests(diff);
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].method_name, "NewTest");
    }

    #[test]
    fn test_extract_namespace_class_from_path() {
        let (ns, class) = extract_namespace_class_from_path("src/Tests/Api/UserTests.cs");
        assert_eq!(class, "UserTests");
        assert!(ns.contains("Tests"));
    }
}
