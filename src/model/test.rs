#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestStatus {
    NotRun,
    Running,
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct Test {
    pub name: String,
    pub full_name: String,
    pub status: TestStatus,
    pub duration_ms: Option<u64>,
    pub error_message: Option<String>,
}

impl Test {
    pub fn new(name: String, full_name: String) -> Self {
        Self {
            name,
            full_name,
            status: TestStatus::NotRun,
            duration_ms: None,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestClass {
    pub name: String,
    pub namespace: String,
    pub tests: Vec<Test>,
}

impl TestClass {
    #[cfg(test)]
    pub fn new(name: String, namespace: String) -> Self {
        Self {
            name,
            namespace,
            tests: Vec::new(),
        }
    }

    pub fn full_name(&self) -> String {
        if self.namespace.is_empty() {
            self.name.clone()
        } else {
            format!("{}.{}", self.namespace, self.name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TestStatus tests
    #[test]
    fn test_status_equality() {
        assert_eq!(TestStatus::NotRun, TestStatus::NotRun);
        assert_eq!(TestStatus::Running, TestStatus::Running);
        assert_eq!(TestStatus::Passed, TestStatus::Passed);
        assert_eq!(TestStatus::Failed, TestStatus::Failed);
        assert_eq!(TestStatus::Skipped, TestStatus::Skipped);
    }

    #[test]
    fn test_status_inequality() {
        assert_ne!(TestStatus::NotRun, TestStatus::Running);
        assert_ne!(TestStatus::Passed, TestStatus::Failed);
        assert_ne!(TestStatus::Running, TestStatus::Skipped);
    }

    #[test]
    fn test_status_clone() {
        let status = TestStatus::Passed;
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }

    #[test]
    fn test_status_debug() {
        assert_eq!(format!("{:?}", TestStatus::NotRun), "NotRun");
        assert_eq!(format!("{:?}", TestStatus::Running), "Running");
        assert_eq!(format!("{:?}", TestStatus::Passed), "Passed");
        assert_eq!(format!("{:?}", TestStatus::Failed), "Failed");
        assert_eq!(format!("{:?}", TestStatus::Skipped), "Skipped");
    }

    // Test struct tests
    #[test]
    fn test_new_creates_test_with_correct_defaults() {
        let test = Test::new("method_name".to_string(), "Namespace.Class.method_name".to_string());

        assert_eq!(test.name, "method_name");
        assert_eq!(test.full_name, "Namespace.Class.method_name");
        assert_eq!(test.status, TestStatus::NotRun);
        assert!(test.duration_ms.is_none());
        assert!(test.error_message.is_none());
    }

    #[test]
    fn test_new_with_empty_name() {
        let test = Test::new(String::new(), String::new());

        assert_eq!(test.name, "");
        assert_eq!(test.full_name, "");
        assert_eq!(test.status, TestStatus::NotRun);
    }

    #[test]
    fn test_clone() {
        let mut test = Test::new("test1".to_string(), "NS.Class.test1".to_string());
        test.status = TestStatus::Passed;
        test.duration_ms = Some(100);
        test.error_message = Some("error".to_string());

        let cloned = test.clone();

        assert_eq!(cloned.name, "test1");
        assert_eq!(cloned.full_name, "NS.Class.test1");
        assert_eq!(cloned.status, TestStatus::Passed);
        assert_eq!(cloned.duration_ms, Some(100));
        assert_eq!(cloned.error_message, Some("error".to_string()));
    }

    #[test]
    fn test_modify_status() {
        let mut test = Test::new("test".to_string(), "test".to_string());

        test.status = TestStatus::Running;
        assert_eq!(test.status, TestStatus::Running);

        test.status = TestStatus::Passed;
        assert_eq!(test.status, TestStatus::Passed);

        test.status = TestStatus::Failed;
        assert_eq!(test.status, TestStatus::Failed);
    }

    #[test]
    fn test_modify_duration() {
        let mut test = Test::new("test".to_string(), "test".to_string());

        test.duration_ms = Some(0);
        assert_eq!(test.duration_ms, Some(0));

        test.duration_ms = Some(1000);
        assert_eq!(test.duration_ms, Some(1000));

        test.duration_ms = Some(u64::MAX);
        assert_eq!(test.duration_ms, Some(u64::MAX));
    }

    #[test]
    fn test_modify_error_message() {
        let mut test = Test::new("test".to_string(), "test".to_string());

        test.error_message = Some("First error".to_string());
        assert_eq!(test.error_message, Some("First error".to_string()));

        test.error_message = None;
        assert!(test.error_message.is_none());
    }

    // TestClass tests
    #[test]
    fn test_class_new() {
        let class = TestClass::new("MyClass".to_string(), "MyNamespace".to_string());

        assert_eq!(class.name, "MyClass");
        assert_eq!(class.namespace, "MyNamespace");
        assert!(class.tests.is_empty());
    }

    #[test]
    fn test_class_new_with_empty_values() {
        let class = TestClass::new(String::new(), String::new());

        assert_eq!(class.name, "");
        assert_eq!(class.namespace, "");
        assert!(class.tests.is_empty());
    }

    #[test]
    fn test_class_full_name_with_namespace() {
        let class = TestClass::new("MyClass".to_string(), "MyNamespace".to_string());
        assert_eq!(class.full_name(), "MyNamespace.MyClass");
    }

    #[test]
    fn test_class_full_name_without_namespace() {
        let class = TestClass::new("MyClass".to_string(), String::new());
        assert_eq!(class.full_name(), "MyClass");
    }

    #[test]
    fn test_class_full_name_nested_namespace() {
        let class = TestClass::new("MyClass".to_string(), "Company.Product.Feature".to_string());
        assert_eq!(class.full_name(), "Company.Product.Feature.MyClass");
    }

    #[test]
    fn test_class_add_tests() {
        let mut class = TestClass::new("MyClass".to_string(), "NS".to_string());

        class.tests.push(Test::new("test1".to_string(), "NS.MyClass.test1".to_string()));
        class.tests.push(Test::new("test2".to_string(), "NS.MyClass.test2".to_string()));

        assert_eq!(class.tests.len(), 2);
        assert_eq!(class.tests[0].name, "test1");
        assert_eq!(class.tests[1].name, "test2");
    }

    #[test]
    fn test_class_clone() {
        let mut class = TestClass::new("MyClass".to_string(), "NS".to_string());
        class.tests.push(Test::new("test1".to_string(), "NS.MyClass.test1".to_string()));

        let cloned = class.clone();

        assert_eq!(cloned.name, "MyClass");
        assert_eq!(cloned.namespace, "NS");
        assert_eq!(cloned.tests.len(), 1);
        assert_eq!(cloned.tests[0].name, "test1");
    }

    #[test]
    fn test_class_debug_output() {
        let class = TestClass::new("MyClass".to_string(), "NS".to_string());
        let debug_str = format!("{:?}", class);

        assert!(debug_str.contains("MyClass"));
        assert!(debug_str.contains("NS"));
    }
}
