use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Represents a test method found in a C# file
#[derive(Debug, Clone)]
pub struct TestMethodInfo {
    pub method_name: String,
    pub class_name: String,
    pub namespace: String,
}

impl TestMethodInfo {
    pub fn full_name(&self) -> String {
        if self.namespace.is_empty() {
            format!("{}.{}", self.class_name, self.method_name)
        } else {
            format!("{}.{}.{}", self.namespace, self.class_name, self.method_name)
        }
    }
}

/// Parse a C# file and extract test methods with their class and namespace info.
pub fn parse_test_file(path: &Path) -> Result<Vec<TestMethodInfo>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    parse_test_content(&content)
}

/// Parse C# content and extract test methods.
pub fn parse_test_content(content: &str) -> Result<Vec<TestMethodInfo>, String> {
    let mut parser = Parser::new();
    let language = tree_sitter_c_sharp::LANGUAGE;
    parser
        .set_language(&language.into())
        .map_err(|e| format!("Failed to set language: {}", e))?;

    let tree = parser
        .parse(content, None)
        .ok_or_else(|| "Failed to parse C# content".to_string())?;

    let mut methods = Vec::new();
    let root = tree.root_node();

    // Find all namespaces and classes
    find_test_methods(&root, content.as_bytes(), "", "", &mut methods);

    Ok(methods)
}

fn find_test_methods(
    node: &Node,
    source: &[u8],
    current_namespace: &str,
    current_class: &str,
    methods: &mut Vec<TestMethodInfo>,
) {
    match node.kind() {
        "compilation_unit" => {
            // For compilation unit, we need to handle file-scoped namespaces specially
            // The namespace applies to all siblings that come after it
            let mut active_namespace = current_namespace.to_string();
            
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "file_scoped_namespace_declaration" {
                        active_namespace = get_namespace_name(&child, source);
                    } else {
                        find_test_methods(&child, source, &active_namespace, current_class, methods);
                    }
                }
            }
        }
        "namespace_declaration" => {
            let ns_name = get_namespace_name(node, source);
            let new_namespace = if current_namespace.is_empty() {
                ns_name
            } else {
                format!("{}.{}", current_namespace, ns_name)
            };

            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    find_test_methods(&child, source, &new_namespace, current_class, methods);
                }
            }
        }
        "class_declaration" => {
            let class_name = get_class_name(node, source);

            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    find_test_methods(&child, source, current_namespace, &class_name, methods);
                }
            }
        }
        "method_declaration" => {
            // Collect ALL methods - dotnet test --list-tests will tell us which are tests
            if let Some(name) = get_method_name(node, source) {
                methods.push(TestMethodInfo {
                    method_name: name,
                    class_name: current_class.to_string(),
                    namespace: current_namespace.to_string(),
                });
            }
        }
        _ => {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    find_test_methods(&child, source, current_namespace, current_class, methods);
                }
            }
        }
    }
}

fn get_namespace_name(node: &Node, source: &[u8]) -> String {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "qualified_name" || child.kind() == "identifier" {
                return node_text(&child, source);
            }
        }
    }
    String::new()
}

fn get_class_name(node: &Node, source: &[u8]) -> String {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" {
                return node_text(&child, source);
            }
        }
    }
    String::new()
}

fn get_method_name(node: &Node, source: &[u8]) -> Option<String> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" {
                return Some(node_text(&child, source));
            }
        }
    }
    None
}

fn node_text(node: &Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}

/// Build a map from method name to full qualified name by parsing C# files in a directory.
pub fn build_test_name_map(project_dir: &Path) -> HashMap<String, TestMethodInfo> {
    let mut map = HashMap::new();

    // Find all .cs files recursively
    if let Ok(entries) = glob_cs_files(project_dir) {
        for path in entries {
            if let Ok(methods) = parse_test_file(&path) {
                for method in methods {
                    // Use method name as key (may have collisions, but that's ok)
                    map.insert(method.method_name.clone(), method);
                }
            }
        }
    }

    map
}

fn glob_cs_files(dir: &Path) -> Result<Vec<std::path::PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    glob_cs_files_recursive(dir, &mut files)?;
    Ok(files)
}

fn glob_cs_files_recursive(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), std::io::Error> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Skip common non-source directories
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dir_name != "obj" && dir_name != "bin" && !dir_name.starts_with('.') {
                glob_cs_files_recursive(&path, files)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "cs") {
            files.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_xunit_test() {
        let content = r#"
namespace MyTests
{
    public class CalculatorTests
    {
        [Fact]
        public void Add_ReturnsSum()
        {
        }

        [Theory]
        public void Add_WithData_ReturnsSum(int a, int b)
        {
        }
    }
}
"#;
        let methods = parse_test_content(content).unwrap();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0].method_name, "Add_ReturnsSum");
        assert_eq!(methods[0].class_name, "CalculatorTests");
        assert_eq!(methods[0].namespace, "MyTests");
        assert_eq!(methods[0].full_name(), "MyTests.CalculatorTests.Add_ReturnsSum");
    }

    #[test]
    fn test_parse_nunit_test() {
        let content = r#"
namespace MyTests
{
    public class StringTests
    {
        [Test]
        public void TestLength()
        {
        }

        [TestCase("hello")]
        public void TestWithCase(string input)
        {
        }
    }
}
"#;
        let methods = parse_test_content(content).unwrap();
        assert_eq!(methods.len(), 2);
    }

    #[test]
    fn test_parse_mstest() {
        let content = r#"
namespace MyTests
{
    public class DataTests
    {
        [TestMethod]
        public void TestSomething()
        {
        }
    }
}
"#;
        let methods = parse_test_content(content).unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].method_name, "TestSomething");
    }

    #[test]
    fn test_nested_namespace() {
        let content = r#"
namespace Company.Product.Tests
{
    public class MyTests
    {
        [Fact]
        public void TestMethod()
        {
        }
    }
}
"#;
        let methods = parse_test_content(content).unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].namespace, "Company.Product.Tests");
        assert_eq!(methods[0].full_name(), "Company.Product.Tests.MyTests.TestMethod");
    }

    #[test]
    fn test_file_scoped_namespace() {
        let content = r#"
namespace MyTests;

public class MyTestClass
{
    [Fact]
    public void TestMethod()
    {
    }
}
"#;
        let methods = parse_test_content(content).unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].namespace, "MyTests");
    }

    #[test]
    fn test_all_methods_collected() {
        // We collect ALL methods - dotnet test --list-tests tells us which are tests
        let content = r#"
namespace MyTests
{
    public class MyTests
    {
        [Fact]
        public void TestMethod()
        {
        }

        public void HelperMethod()
        {
        }

        private void SetUp()
        {
        }
    }
}
"#;
        let methods = parse_test_content(content).unwrap();
        // All 3 methods are collected
        assert_eq!(methods.len(), 3);
        let names: Vec<_> = methods.iter().map(|m| m.method_name.as_str()).collect();
        assert!(names.contains(&"TestMethod"));
        assert!(names.contains(&"HelperMethod"));
        assert!(names.contains(&"SetUp"));
    }
}
