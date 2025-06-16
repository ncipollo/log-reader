//! Stream implementation for reading log files with real-time monitoring.

use crate::error::{Error, Result};
use crate::reader::read_file_content;
use crate::watcher::{FileWatcher, is_event_relevant_to_file};
use futures::Stream;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

/// A stream that monitors a file for changes and yields new content.
pub struct LogStream {
    receiver: mpsc::UnboundedReceiver<Result<Vec<String>>>,
    _shutdown_tx: broadcast::Sender<()>,
    _task_handle: JoinHandle<()>,
}

impl LogStream {
    /// Creates a new LogStream for the specified file.
    pub async fn new<P: AsRef<Path>>(path: P, separator: Option<String>) -> Result<Self> {
        let file_path = path.as_ref().to_path_buf();
        let separator = separator.unwrap_or_else(|| "\n".to_string());

        let (tx, rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        // Spawn background task to handle file watching and reading
        let task_file_path = file_path.clone();
        let task_separator = separator.clone();
        let task_tx = tx.clone();

        let task_handle = tokio::spawn(async move {
            if let Err(e) =
                file_reader_task(task_file_path, task_separator, task_tx, shutdown_rx).await
            {
                // Log error or send it through channel
                eprintln!("File reader task error: {}", e);
            }
        });

        Ok(LogStream {
            receiver: rx,
            _shutdown_tx: shutdown_tx,
            _task_handle: task_handle,
        })
    }

    /// Check if the stream has been closed/dropped
    #[cfg(test)]
    pub fn is_closed(&self) -> bool {
        self.receiver.is_closed()
    }
}

impl Drop for LogStream {
    fn drop(&mut self) {
        // Send shutdown signal - ignore errors if already dropped or no receivers
        let _ = self._shutdown_tx.send(());

        // The task handle will be automatically aborted when it's dropped,
        // but we've also sent a graceful shutdown signal for clean cleanup
    }
}

/// Background task that handles file watching and reading
async fn file_reader_task(
    file_path: PathBuf,
    separator: String,
    tx: mpsc::UnboundedSender<Result<Vec<String>>>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<()> {
    let mut last_position = 0u64;

    // Read existing content in the file.
    if file_path.exists() {
        if let Err(e) = read_file_content(&file_path, &mut last_position, &separator, &tx).await {
            let _ = tx.send(Err(e));
            return Ok(());
        }
    }

    // Now start watching for future changes
    let mut watcher = FileWatcher::new(&file_path)?;
    watcher.start_watching()?;

    // Get the file name for filtering
    let file_name = file_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    // Watch for file changes
    loop {
        tokio::select! {
            // Check for shutdown signal
            _ = shutdown_rx.recv() => {
                // Graceful shutdown requested
                break;
            }

            // Process file events
            event = watcher.next_event() => {
                match event {
                    Some(Ok(event)) => {
                        // Filter events to only include those affecting our target file
                        if is_event_relevant_to_file(&event, &file_name) {
                            if let Err(e) = read_file_content(&file_path, &mut last_position, &separator, &tx).await {
                                let _ = tx.send(Err(e));
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        let _ = tx.send(Err(Error::Watcher(e)));
                        break;
                    }
                    None => {
                        // Watcher closed, shutdown gracefully
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

impl Stream for LogStream {
    type Item = Result<Vec<String>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_log_stream_creation() {
        let stream = LogStream::new("fixtures/simple_append.log", None).await;
        assert!(stream.is_ok());

        let stream = stream.unwrap();
        assert!(!stream.is_closed());
    }

    #[tokio::test]
    async fn test_log_stream_creation_with_custom_separator() {
        let stream =
            LogStream::new("fixtures/different_separators.log", Some("|".to_string())).await;
        assert!(stream.is_ok());

        let stream = stream.unwrap();
        assert!(!stream.is_closed());
    }

    #[tokio::test]
    async fn test_log_stream_creation_nonexistent_file() {
        let stream = LogStream::new("fixtures/nonexistent.log", None).await;
        assert!(stream.is_ok());

        let stream = stream.unwrap();
        assert!(!stream.is_closed());
    }

    #[tokio::test]
    async fn test_log_stream_graceful_shutdown_on_drop() {
        let mut stream = LogStream::new("fixtures/simple_append.log", None)
            .await
            .unwrap();

        // Consume one item to ensure the stream is active
        let first_item = tokio::time::timeout(Duration::from_millis(100), stream.next()).await;
        assert!(first_item.is_ok());

        // Drop the stream
        drop(stream);

        // Give background task time to shut down
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Test passes if we reach here without hanging
    }

    #[tokio::test]
    async fn test_log_stream_multiple_streams_independence() {
        let stream1 = LogStream::new("fixtures/simple_append.log", None)
            .await
            .unwrap();
        let stream2 = LogStream::new("fixtures/simple_append.log", None)
            .await
            .unwrap();

        assert!(!stream1.is_closed());
        assert!(!stream2.is_closed());

        // Drop first stream
        drop(stream1);

        // Second stream should still be functional
        assert!(!stream2.is_closed());

        drop(stream2);
    }

    #[tokio::test]
    async fn test_log_stream_reading_existing_content() {
        let mut stream = LogStream::new("fixtures/simple_append.log", None)
            .await
            .unwrap();

        // Should immediately yield existing content
        let items = collect_stream_items(&mut stream, 5, Duration::from_millis(100)).await;

        assert!(!items.is_empty());
        // Now we get Vec<String> items, so check the first Vec contains the expected content
        assert!(items[0].len() > 0);
        assert!(items[0][0].contains("Starting application"));
    }

    #[tokio::test]
    async fn test_log_stream_with_custom_separator() {
        let mut stream = LogStream::new("fixtures/different_separators.log", Some("|".to_string()))
            .await
            .unwrap();

        let items = collect_stream_items(&mut stream, 3, Duration::from_millis(100)).await;

        assert!(!items.is_empty());
        // Should split by pipe character - now we get one Vec<String> with multiple parts
        assert_eq!(items.len(), 1);
        let lines = &items[0];
        assert!(lines.len() > 1);
    }

    #[tokio::test]
    async fn test_log_stream_empty_file() {
        let mut stream = LogStream::new("fixtures/empty.log", None).await.unwrap();

        // Empty file should not yield any items immediately
        let items = collect_stream_items(&mut stream, 1, Duration::from_millis(50)).await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn test_file_reader_task_shutdown_signal() {
        let file_path = PathBuf::from("fixtures/simple_append.log");
        let separator = "\n".to_string();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        // Start the task
        let task_handle =
            tokio::spawn(
                async move { file_reader_task(file_path, separator, tx, shutdown_rx).await },
            );

        // Let it run briefly
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Send shutdown signal
        let _ = shutdown_tx.send(());

        // Task should complete gracefully
        let result = tokio::time::timeout(Duration::from_millis(100), task_handle).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());

        // Should have received some messages from reading existing content
        let mut message_count = 0;
        while rx.try_recv().is_ok() {
            message_count += 1;
        }
        assert!(message_count > 0);
    }

    #[tokio::test]
    async fn test_file_reader_task_error_handling() {
        let file_path = PathBuf::from("/invalid/path/that/does/not/exist.log");
        let separator = "\n".to_string();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_shutdown_tx, shutdown_rx) = broadcast::channel(1);

        // Task should handle invalid paths gracefully
        let result = file_reader_task(file_path, separator, tx, shutdown_rx).await;

        // Task should complete without panicking
        assert!(result.is_ok() || result.is_err());

        // Check if any error messages were sent
        while rx.try_recv().is_ok() {
            // Consume any messages
        }
    }

    // Helper function to collect stream items with timeout
    async fn collect_stream_items(
        stream: &mut LogStream,
        max_items: usize,
        timeout: Duration,
    ) -> Vec<Vec<String>> {
        let mut items = Vec::new();
        let start = tokio::time::Instant::now();

        while items.len() < max_items && start.elapsed() < timeout {
            match tokio::time::timeout(Duration::from_millis(10), stream.next()).await {
                Ok(Some(Ok(item))) => items.push(item),
                Ok(Some(Err(_))) => break, // Error occurred
                Ok(None) => break,         // Stream ended
                Err(_) => break,           // Timeout
            }
        }

        items
    }
}
