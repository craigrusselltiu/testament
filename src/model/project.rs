use std::path::PathBuf;

use super::TestClass;

#[derive(Debug, Clone)]
pub struct TestProject {
    pub name: String,
    pub path: PathBuf,
    pub classes: Vec<TestClass>,
}

impl TestProject {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            classes: Vec::new(),
        }
    }

    pub fn test_count(&self) -> usize {
        self.classes.iter().map(|c| c.tests.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Test;

    #[test]
    fn test_project_new() {
        let path = PathBuf::from("/path/to/project.csproj");
        let project = TestProject::new("MyProject".to_string(), path.clone());

        assert_eq!(project.name, "MyProject");
        assert_eq!(project.path, path);
        assert!(project.classes.is_empty());
    }

    #[test]
    fn test_project_new_with_empty_name() {
        let path = PathBuf::from("");
        let project = TestProject::new(String::new(), path.clone());

        assert_eq!(project.name, "");
        assert_eq!(project.path, path);
    }

    #[test]
    fn test_project_test_count_empty() {
        let project = TestProject::new("Project".to_string(), PathBuf::from("/test"));
        assert_eq!(project.test_count(), 0);
    }

    #[test]
    fn test_project_test_count_single_class_single_test() {
        let mut project = TestProject::new("Project".to_string(), PathBuf::from("/test"));
        let mut class = TestClass::new("Class1".to_string(), "NS".to_string());
        class.tests.push(Test::new("test1".to_string(), "NS.Class1.test1".to_string()));
        project.classes.push(class);

        assert_eq!(project.test_count(), 1);
    }

    #[test]
    fn test_project_test_count_single_class_multiple_tests() {
        let mut project = TestProject::new("Project".to_string(), PathBuf::from("/test"));
        let mut class = TestClass::new("Class1".to_string(), "NS".to_string());
        class.tests.push(Test::new("test1".to_string(), "NS.Class1.test1".to_string()));
        class.tests.push(Test::new("test2".to_string(), "NS.Class1.test2".to_string()));
        class.tests.push(Test::new("test3".to_string(), "NS.Class1.test3".to_string()));
        project.classes.push(class);

        assert_eq!(project.test_count(), 3);
    }

    #[test]
    fn test_project_test_count_multiple_classes() {
        let mut project = TestProject::new("Project".to_string(), PathBuf::from("/test"));

        let mut class1 = TestClass::new("Class1".to_string(), "NS".to_string());
        class1.tests.push(Test::new("test1".to_string(), "NS.Class1.test1".to_string()));
        class1.tests.push(Test::new("test2".to_string(), "NS.Class1.test2".to_string()));
        project.classes.push(class1);

        let mut class2 = TestClass::new("Class2".to_string(), "NS".to_string());
        class2.tests.push(Test::new("test1".to_string(), "NS.Class2.test1".to_string()));
        project.classes.push(class2);

        let mut class3 = TestClass::new("Class3".to_string(), "NS".to_string());
        class3.tests.push(Test::new("test1".to_string(), "NS.Class3.test1".to_string()));
        class3.tests.push(Test::new("test2".to_string(), "NS.Class3.test2".to_string()));
        class3.tests.push(Test::new("test3".to_string(), "NS.Class3.test3".to_string()));
        project.classes.push(class3);

        assert_eq!(project.test_count(), 6);
    }

    #[test]
    fn test_project_test_count_with_empty_class() {
        let mut project = TestProject::new("Project".to_string(), PathBuf::from("/test"));
        let class = TestClass::new("EmptyClass".to_string(), "NS".to_string());
        project.classes.push(class);

        assert_eq!(project.test_count(), 0);
    }

    #[test]
    fn test_project_test_count_mixed_empty_and_nonempty_classes() {
        let mut project = TestProject::new("Project".to_string(), PathBuf::from("/test"));

        let empty_class = TestClass::new("EmptyClass".to_string(), "NS".to_string());
        project.classes.push(empty_class);

        let mut nonempty_class = TestClass::new("TestClass".to_string(), "NS".to_string());
        nonempty_class.tests.push(Test::new("test1".to_string(), "NS.TestClass.test1".to_string()));
        nonempty_class.tests.push(Test::new("test2".to_string(), "NS.TestClass.test2".to_string()));
        project.classes.push(nonempty_class);

        assert_eq!(project.test_count(), 2);
    }

    #[test]
    fn test_project_clone() {
        let mut project = TestProject::new("Project".to_string(), PathBuf::from("/test/project.csproj"));
        let mut class = TestClass::new("Class1".to_string(), "NS".to_string());
        class.tests.push(Test::new("test1".to_string(), "NS.Class1.test1".to_string()));
        project.classes.push(class);

        let cloned = project.clone();

        assert_eq!(cloned.name, "Project");
        assert_eq!(cloned.path, PathBuf::from("/test/project.csproj"));
        assert_eq!(cloned.classes.len(), 1);
        assert_eq!(cloned.test_count(), 1);
    }

    #[test]
    fn test_project_debug_output() {
        let project = TestProject::new("MyProject".to_string(), PathBuf::from("/path/to/proj.csproj"));
        let debug_str = format!("{:?}", project);

        assert!(debug_str.contains("MyProject"));
        assert!(debug_str.contains("proj.csproj") || debug_str.contains("path"));
    }

    #[test]
    fn test_project_path_with_spaces() {
        let path = PathBuf::from("/path/with spaces/project.csproj");
        let project = TestProject::new("Project".to_string(), path.clone());

        assert_eq!(project.path, path);
    }

    #[test]
    fn test_project_path_with_unicode() {
        let path = PathBuf::from("/path/日本語/проект.csproj");
        let project = TestProject::new("Project".to_string(), path.clone());

        assert_eq!(project.path, path);
    }
}
