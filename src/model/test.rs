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
