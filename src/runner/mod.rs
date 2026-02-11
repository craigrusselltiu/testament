mod discovery;
mod executor;
mod watcher;

pub use discovery::{discover_projects_lazy, discover_projects_from_paths, find_solution, find_csproj_in_dir, DiscoveryEvent};
pub use executor::{ExecutorEvent, TestExecutor};
pub use watcher::FileWatcher;
