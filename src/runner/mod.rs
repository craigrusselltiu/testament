mod discovery;
mod executor;
mod watcher;

pub use discovery::{discover_projects, find_solution};
pub use executor::{ExecutorEvent, TestExecutor};
pub use watcher::FileWatcher;
