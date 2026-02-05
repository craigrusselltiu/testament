mod discovery;
mod executor;
mod watcher;

pub use discovery::{discover_projects_lazy, find_solution, DiscoveryEvent};
pub use executor::{ExecutorEvent, TestExecutor};
pub use watcher::FileWatcher;
