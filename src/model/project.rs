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
