mod discovery;
mod executor;

pub use discovery::{discover_projects, find_solution};
pub use executor::{run_tests, ExecutorEvent, TestExecutor};
