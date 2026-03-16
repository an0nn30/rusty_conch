//! File watching for live-reload of config and themes.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum FileChangeKind {
    Config,
    Themes,
}

pub(crate) struct FileChange {
    pub kind: FileChangeKind,
    pub path: Option<PathBuf>,
}

pub(crate) struct FileWatcher {
    rx: mpsc::Receiver<notify::Result<Event>>,
    _watcher: RecommendedWatcher,
    config_path: PathBuf,
    themes_dir: PathBuf,
    last_event: std::collections::HashMap<FileChangeKind, Instant>,
}

const DEBOUNCE: Duration = Duration::from_secs(1);

impl FileWatcher {
    pub(crate) fn start() -> Option<Self> {
        let (tx, rx) = mpsc::channel();

        let mut watcher = match RecommendedWatcher::new(
            move |res| { let _ = tx.send(res); },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                log::warn!("Failed to create file watcher: {e}");
                return None;
            }
        };

        let config_dir = conch_core::config::config_dir();
        let config_path = conch_core::config::config_path();
        let themes_dir = config_dir.join("themes");

        if !config_dir.exists() {
            let _ = std::fs::create_dir_all(&config_dir);
        }

        if let Err(e) = watcher.watch(&config_dir, RecursiveMode::Recursive) {
            log::warn!("Cannot watch config dir {}: {e}", config_dir.display());
        }

        Some(Self {
            rx,
            _watcher: watcher,
            config_path,
            themes_dir,
            last_event: std::collections::HashMap::new(),
        })
    }

    fn classify(&self, path: &std::path::Path) -> Option<FileChangeKind> {
        if path == self.config_path {
            return Some(FileChangeKind::Config);
        }
        if path.starts_with(&self.themes_dir) {
            return Some(FileChangeKind::Themes);
        }
        None
    }

    pub(crate) fn poll(&mut self) -> Vec<FileChange> {
        let now = Instant::now();
        let mut triggered: std::collections::HashMap<FileChangeKind, Option<PathBuf>> =
            std::collections::HashMap::new();

        while let Ok(event_result) = self.rx.try_recv() {
            let Ok(event) = event_result else { continue };

            for path in &event.paths {
                if let Some(kind) = self.classify(path) {
                    let should_emit = self
                        .last_event
                        .get(&kind)
                        .map(|last| now.duration_since(*last) >= DEBOUNCE)
                        .unwrap_or(true);
                    if should_emit {
                        triggered.insert(kind.clone(), Some(path.clone()));
                        self.last_event.insert(kind, now);
                    }
                }
            }
        }

        triggered
            .into_iter()
            .map(|(kind, path)| FileChange { kind, path })
            .collect()
    }
}
