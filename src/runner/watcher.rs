use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, Event};

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    rx: Receiver<()>,
}

impl FileWatcher {
    pub fn new(watch_path: &Path) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let debounce_duration = Duration::from_millis(500);
        let mut last_event = Instant::now() - debounce_duration;

        let mut watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                if let Ok(event) = result {
                    // Only trigger on file modifications
                    if event.kind.is_modify() || event.kind.is_create() {
                        // Check for relevant file extensions
                        let is_relevant = event.paths.iter().any(|p| {
                            p.extension()
                                .map(|ext| ext == "cs" || ext == "csproj")
                                .unwrap_or(false)
                        });

                        if is_relevant {
                            let now = Instant::now();
                            if now.duration_since(last_event) >= debounce_duration {
                                last_event = now;
                                let _ = tx.send(());
                            }
                        }
                    }
                }
            },
            Config::default(),
        )?;

        watcher.watch(watch_path, RecursiveMode::Recursive)?;

        Ok(Self {
            _watcher: watcher,
            rx,
        })
    }

    pub fn try_recv(&self) -> bool {
        self.rx.try_recv().is_ok()
    }
}
