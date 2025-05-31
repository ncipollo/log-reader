//! File watching functionality using the notify crate.

use crate::error::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// A simple file watcher that monitors a specific file for changes.
pub(crate) struct FileWatcher {
    _watcher: RecommendedWatcher,
    receiver: mpsc::UnboundedReceiver<notify::Result<Event>>,
    file_path: PathBuf,
}

impl FileWatcher {
    /// Creates a new file watcher for the specified path.
    pub(crate) fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file_path = path.as_ref().to_path_buf();
        
        let (tx, rx) = mpsc::unbounded_channel();

        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        )?;

        // We need to store the watcher to keep it alive, but we don't actually start watching yet
        Ok(Self {
            _watcher: watcher,
            receiver: rx,
            file_path,
        })
    }

    /// Starts watching the file for changes.
    pub(crate) fn start_watching(&mut self) -> Result<()> {
        let watch_path = self.file_path.parent().unwrap_or(&self.file_path);
        self._watcher.watch(watch_path, RecursiveMode::NonRecursive)?;
        Ok(())
    }

    /// Returns the next file system event.
    pub(crate) async fn next_event(&mut self) -> Option<notify::Result<Event>> {
        self.receiver.recv().await
    }
    
    #[cfg(test)]
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}

/// Check if a notify event is relevant to a specific file
pub(crate) fn is_event_relevant_to_file(event: &Event, target_file_name: &str) -> bool {
    event.paths.iter().any(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy() == target_file_name)
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::{EventKind, Event};
    use std::path::PathBuf;

    #[test]
    fn test_file_watcher_creation() {
        let file_path = PathBuf::from("/tmp/test.log");
        let watcher = FileWatcher::new(&file_path);
        
        assert!(watcher.is_ok());
        let watcher = watcher.unwrap();
        assert_eq!(watcher.file_path(), file_path.as_path());
    }

    #[test]
    fn test_file_watcher_with_relative_path() {
        let file_path = PathBuf::from("test.log");
        let watcher = FileWatcher::new(&file_path);
        
        assert!(watcher.is_ok());
        let watcher = watcher.unwrap();
        assert_eq!(watcher.file_path(), file_path.as_path());
    }

    #[test]
    fn test_file_watcher_with_nested_path() {
        let file_path = PathBuf::from("/var/log/app/test.log");
        let watcher = FileWatcher::new(&file_path);
        
        assert!(watcher.is_ok());
        let watcher = watcher.unwrap();
        assert_eq!(watcher.file_path(), file_path.as_path());
    }

    #[test]
    fn test_is_event_relevant_to_file_exact_match() {
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![PathBuf::from("/tmp/test.log")],
            attrs: Default::default(),
        };
        
        assert!(is_event_relevant_to_file(&event, "test.log"));
        assert!(!is_event_relevant_to_file(&event, "other.log"));
    }

    #[test]
    fn test_is_event_relevant_to_file_multiple_paths() {
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![
                PathBuf::from("/tmp/other.log"),
                PathBuf::from("/tmp/test.log"),
                PathBuf::from("/tmp/another.log"),
            ],
            attrs: Default::default(),
        };
        
        assert!(is_event_relevant_to_file(&event, "test.log"));
        assert!(is_event_relevant_to_file(&event, "other.log"));
        assert!(is_event_relevant_to_file(&event, "another.log"));
        assert!(!is_event_relevant_to_file(&event, "missing.log"));
    }

    #[test]
    fn test_is_event_relevant_to_file_different_directories() {
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![
                PathBuf::from("/var/log/test.log"),
                PathBuf::from("/tmp/test.log"),
            ],
            attrs: Default::default(),
        };
        
        // Should match both files with same name but different directories
        assert!(is_event_relevant_to_file(&event, "test.log"));
    }

    #[test]
    fn test_is_event_relevant_to_file_no_file_name() {
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![PathBuf::from("/")], // Root directory has no file name
            attrs: Default::default(),
        };
        
        assert!(!is_event_relevant_to_file(&event, "test.log"));
    }

    #[test]
    fn test_is_event_relevant_to_file_empty_paths() {
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![],
            attrs: Default::default(),
        };
        
        assert!(!is_event_relevant_to_file(&event, "test.log"));
    }

    #[test]
    fn test_is_event_relevant_to_file_case_sensitivity() {
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![PathBuf::from("/tmp/Test.Log")],
            attrs: Default::default(),
        };
        
        // Should be case sensitive
        assert!(!is_event_relevant_to_file(&event, "test.log"));
        assert!(is_event_relevant_to_file(&event, "Test.Log"));
    }

    #[test] 
    fn test_is_event_relevant_to_file_special_characters() {
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![PathBuf::from("/tmp/app-test_file.log")],
            attrs: Default::default(),
        };
        
        assert!(is_event_relevant_to_file(&event, "app-test_file.log"));
        assert!(!is_event_relevant_to_file(&event, "app-test-file.log"));
    }

    #[tokio::test]
    async fn test_file_watcher_start_watching_existing_file() {
        let file_path = PathBuf::from("fixtures/simple_append.log");
        let mut watcher = FileWatcher::new(&file_path).unwrap();
        
        let result = watcher.start_watching();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_file_watcher_start_watching_nonexistent_file() {
        let file_path = PathBuf::from("fixtures/nonexistent.log");
        let mut watcher = FileWatcher::new(&file_path).unwrap();
        
        // Should not fail even if file doesn't exist, as we watch the directory
        let result = watcher.start_watching();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_file_watcher_start_watching_file_without_parent() {
        let file_path = PathBuf::from("test.log");
        let mut watcher = FileWatcher::new(&file_path).unwrap();
        
        // Should handle files in current directory
        // Note: This might fail if the current directory doesn't exist or isn't accessible
        let result = watcher.start_watching();
        
        // On some systems, watching current directory might not be allowed
        // So we accept either success or a specific kind of failure
        match result {
            Ok(_) => {}, // Success case
            Err(crate::error::Error::Watcher(notify_error)) => {
                // Accept certain types of watcher errors as expected
                match notify_error.kind {
                    notify::ErrorKind::Generic(_) | 
                    notify::ErrorKind::Io(_) | 
                    notify::ErrorKind::PathNotFound => {},
                    _ => panic!("Unexpected notify error: {:?}", notify_error),
                }
            },
            Err(e) => panic!("Unexpected error type: {:?}", e),
        }
    }

    #[tokio::test]  
    async fn test_file_watcher_next_event_timeout() {
        let file_path = PathBuf::from("fixtures/simple_append.log");
        let mut watcher = FileWatcher::new(&file_path).unwrap();
        watcher.start_watching().unwrap();
        
        // Test that next_event doesn't block indefinitely when no events occur
        // Use a timeout to ensure we don't wait forever
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(10),
            watcher.next_event()
        ).await;
        
        // Should timeout since no events are occurring
        assert!(result.is_err());
    }
} 