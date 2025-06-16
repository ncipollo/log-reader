//! File reading utilities for log processing.

use crate::error::Result;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::mpsc;

/// Read content from file and send lines through the channel
pub(crate) async fn read_file_content(
    file_path: &Path,
    last_position: &mut u64,
    separator: &str,
    tx: &mpsc::UnboundedSender<Result<Vec<String>>>,
) -> Result<()> {
    if !file_path.exists() {
        return Ok(());
    }

    let mut file = File::open(file_path).await?;
    let metadata = file.metadata().await?;
    let current_size = metadata.len();

    // Handle file truncation
    if detect_file_truncation(current_size, *last_position) {
        *last_position = 0;
    }

    // Check if there's new content to read
    let bytes_to_read = match calculate_bytes_to_read(current_size, *last_position) {
        Some(bytes) => bytes,
        None => return Ok(()), // Nothing new to read
    };

    // Seek to last known position
    file.seek(std::io::SeekFrom::Start(*last_position)).await?;

    // Read new content
    let mut new_content = String::new();
    file.take(bytes_to_read)
        .read_to_string(&mut new_content)
        .await?;

    // Update position
    *last_position = current_size;

    // Split by separator and collect all parts into a Vec
    let parts = split_and_filter_content(&new_content, separator);

    // Send the entire Vec if it's not empty
    if !parts.is_empty() {
        let _ = tx.send(Ok(parts));
    }

    Ok(())
}

/// Split content by separator and filter out empty/whitespace-only parts
fn split_and_filter_content(content: &str, separator: &str) -> Vec<String> {
    content
        .split(separator)
        .filter_map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(part.to_string())
            }
        })
        .collect()
}

/// Detect if the file was truncated by comparing current size with last position
fn detect_file_truncation(current_size: u64, last_position: u64) -> bool {
    current_size < last_position
}

/// Calculate bytes to read based on current size and last position
fn calculate_bytes_to_read(current_size: u64, last_position: u64) -> Option<u64> {
    if current_size <= last_position {
        None // Nothing new to read
    } else {
        Some(current_size - last_position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::fs;
    use tokio::sync::mpsc;

    /// Helper function to collect all messages from the receiver
    async fn collect_messages(
        mut rx: mpsc::UnboundedReceiver<Result<Vec<String>>>,
    ) -> Vec<Vec<String>> {
        let mut messages = Vec::new();

        // Use try_recv to avoid blocking - all messages should be available immediately
        while let Ok(result) = rx.try_recv() {
            match result {
                Ok(content) => messages.push(content),
                Err(e) => panic!("Unexpected error: {}", e),
            }
        }

        messages
    }

    // Tests for the new pure functions
    #[test]
    fn test_split_and_filter_content_newline() {
        let content = "line1\nline2\nline3\n";
        let result = split_and_filter_content(content, "\n");
        assert_eq!(result, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_split_and_filter_content_with_empty_lines() {
        let content = "line1\n\n\nline2\n  \n\nline3\n";
        let result = split_and_filter_content(content, "\n");
        assert_eq!(result, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_split_and_filter_content_custom_separator() {
        let content = "data1|data2|data3|";
        let result = split_and_filter_content(content, "|");
        assert_eq!(result, vec!["data1", "data2", "data3"]);
    }

    #[test]
    fn test_split_and_filter_content_multi_char_separator() {
        let content = "part1<<>>part2<<>>part3<<>>";
        let result = split_and_filter_content(content, "<<>>");
        assert_eq!(result, vec!["part1", "part2", "part3"]);
    }

    #[test]
    fn test_split_and_filter_content_no_separator() {
        let content = "single_line_content";
        let result = split_and_filter_content(content, "\n");
        assert_eq!(result, vec!["single_line_content"]);
    }

    #[test]
    fn test_split_and_filter_content_empty_string() {
        let content = "";
        let result = split_and_filter_content(content, "\n");
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_split_and_filter_content_only_separators() {
        let content = "\n\n\n";
        let result = split_and_filter_content(content, "\n");
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_split_and_filter_content_whitespace_preservation() {
        let content = "  line1  \n  line2  \n";
        let result = split_and_filter_content(content, "\n");
        // Should preserve internal whitespace but filter empty lines
        assert_eq!(result, vec!["  line1  ", "  line2  "]);
    }

    #[test]
    fn test_detect_file_truncation() {
        assert!(detect_file_truncation(100, 200)); // File was truncated
        assert!(!detect_file_truncation(200, 100)); // File grew
        assert!(!detect_file_truncation(100, 100)); // No change
    }

    #[test]
    fn test_calculate_bytes_to_read() {
        assert_eq!(calculate_bytes_to_read(200, 100), Some(100)); // 100 new bytes
        assert_eq!(calculate_bytes_to_read(100, 100), None); // No new bytes
        assert_eq!(calculate_bytes_to_read(50, 100), None); // File truncated
        assert_eq!(calculate_bytes_to_read(0, 0), None); // Empty file, no change
    }

    // Integration tests for read_file_content function
    #[tokio::test]
    async fn test_read_simple_file_with_newline_separator() {
        let file_path = PathBuf::from("fixtures/simple_append.log");
        let (tx, rx) = mpsc::unbounded_channel();
        let mut position = 0u64;

        read_file_content(&file_path, &mut position, "\n", &tx)
            .await
            .expect("Should read file successfully");

        let messages = collect_messages(rx).await;

        // Should have received exactly one Vec<String> with all lines
        assert_eq!(messages.len(), 1);

        let lines = &messages[0];

        // Expected all 10 lines from the fixture
        let expected = vec![
            "2023-01-01 10:00:00 INFO Starting application".to_string(),
            "2023-01-01 10:00:01 INFO Loading configuration".to_string(),
            "2023-01-01 10:00:02 INFO Database connection established".to_string(),
            "2023-01-01 10:00:03 DEBUG User session created for user_id=123".to_string(),
            "2023-01-01 10:00:04 INFO Application ready to serve requests".to_string(),
            "2023-01-01 10:00:05 WARN High memory usage detected: 85%".to_string(),
            "2023-01-01 10:00:06 ERROR Failed to process request: timeout".to_string(),
            "2023-01-01 10:00:07 INFO Request processed successfully".to_string(),
            "2023-01-01 10:00:08 DEBUG Cache hit for key=user_data_123".to_string(),
            "2023-01-01 10:00:09 INFO User authenticated successfully ".to_string(),
        ];

        assert_eq!(lines, &expected);

        // Position should be at the end of file
        let metadata = fs::metadata(&file_path).await.unwrap();
        assert_eq!(position, metadata.len());
    }

    #[tokio::test]
    async fn test_read_file_with_different_separator() {
        let file_path = PathBuf::from("fixtures/different_separators.log");
        let (tx, rx) = mpsc::unbounded_channel();
        let mut position = 0u64;

        read_file_content(&file_path, &mut position, "|", &tx)
            .await
            .expect("Should read file successfully");

        let messages = collect_messages(rx).await;

        // Should have received exactly one Vec<String> with all parts
        assert_eq!(messages.len(), 1);

        let lines = &messages[0];

        // Should split by pipe character
        assert!(lines.len() > 1);
        assert!(lines[0].contains("Starting application"));
        assert!(lines[1].contains("Loading configuration"));

        let metadata = fs::metadata(&file_path).await.unwrap();
        assert_eq!(position, metadata.len());
    }

    #[tokio::test]
    async fn test_incremental_reading() {
        let file_path = PathBuf::from("fixtures/simple_append.log");
        let (tx, rx) = mpsc::unbounded_channel();

        // First read - only read first 50 bytes to simulate partial reading
        let file = File::open(&file_path).await.unwrap();
        let first_chunk_size = 50;
        let mut content = String::new();
        file.take(first_chunk_size)
            .read_to_string(&mut content)
            .await
            .unwrap();
        let mut position = first_chunk_size;

        // Now read from position 50 to end
        read_file_content(&file_path, &mut position, "\n", &tx)
            .await
            .expect("Should read remaining content");

        let messages = collect_messages(rx).await;

        // Should have received exactly one Vec<String> with remaining lines
        assert_eq!(messages.len(), 1);

        let lines = &messages[0];

        // Expected messages when reading from position 50 onwards
        let expected = vec![
            "-01-01 10:00:01 INFO Loading configuration".to_string(),
            "2023-01-01 10:00:02 INFO Database connection established".to_string(),
            "2023-01-01 10:00:03 DEBUG User session created for user_id=123".to_string(),
            "2023-01-01 10:00:04 INFO Application ready to serve requests".to_string(),
            "2023-01-01 10:00:05 WARN High memory usage detected: 85%".to_string(),
            "2023-01-01 10:00:06 ERROR Failed to process request: timeout".to_string(),
            "2023-01-01 10:00:07 INFO Request processed successfully".to_string(),
            "2023-01-01 10:00:08 DEBUG Cache hit for key=user_data_123".to_string(),
            "2023-01-01 10:00:09 INFO User authenticated successfully ".to_string(),
        ];

        assert_eq!(lines, &expected);

        // Position should be at end of file
        let metadata = fs::metadata(&file_path).await.unwrap();
        assert_eq!(position, metadata.len());
    }

    #[tokio::test]
    async fn test_file_truncation_handling() {
        let file_path = PathBuf::from("fixtures/simple_append.log");
        let (tx, rx) = mpsc::unbounded_channel();
        let mut position = 1000u64; // Set position beyond file size

        read_file_content(&file_path, &mut position, "\n", &tx)
            .await
            .expect("Should handle truncation");

        let messages = collect_messages(rx).await;

        // Should read all content from beginning due to truncation detection
        assert!(messages.len() > 0);

        // Position should be reset and then set to end of file
        let metadata = fs::metadata(&file_path).await.unwrap();
        assert_eq!(position, metadata.len());
    }

    #[tokio::test]
    async fn test_nonexistent_file() {
        let file_path = PathBuf::from("fixtures/nonexistent.log");
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut position = 0u64;

        let result = read_file_content(&file_path, &mut position, "\n", &tx).await;

        // Should not error for non-existent file
        assert!(result.is_ok());
        assert_eq!(position, 0);
    }

    #[tokio::test]
    async fn test_empty_file() {
        let file_path = PathBuf::from("fixtures/empty.log");
        let (tx, rx) = mpsc::unbounded_channel();
        let mut position = 0u64;

        read_file_content(&file_path, &mut position, "\n", &tx)
            .await
            .expect("Should handle empty file");

        let messages = collect_messages(rx).await;

        // Empty file should produce no messages
        assert_eq!(messages.len(), 0);

        // Position should match file size (which is minimal for empty file)
        let metadata = fs::metadata(&file_path).await.unwrap();
        assert_eq!(position, metadata.len());
    }

    #[tokio::test]
    async fn test_no_new_content_when_position_at_end() {
        let file_path = PathBuf::from("fixtures/simple_append.log");
        let (tx, rx) = mpsc::unbounded_channel();

        // Set position to file size (at end)
        let metadata = fs::metadata(&file_path).await.unwrap();
        let mut position = metadata.len();

        read_file_content(&file_path, &mut position, "\n", &tx)
            .await
            .expect("Should handle no new content");

        let messages = collect_messages(rx).await;

        // Should produce no messages when already at end
        assert_eq!(messages.len(), 0);
        assert_eq!(position, metadata.len());
    }

    #[tokio::test]
    async fn test_receiver_dropped() {
        let file_path = PathBuf::from("fixtures/simple_append.log");
        let (tx, rx) = mpsc::unbounded_channel();
        let mut position = 0u64;

        // Drop the receiver to simulate channel closure
        drop(rx);

        // Should not panic and should complete successfully
        let result = read_file_content(&file_path, &mut position, "\n", &tx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_filters_empty_lines() {
        // Create a temporary file with empty lines
        let temp_file = "test_empty_lines.tmp";
        fs::write(temp_file, "line1\n\n\nline2\n  \n\nline3\n")
            .await
            .unwrap();

        let file_path = PathBuf::from(temp_file);
        let (tx, rx) = mpsc::unbounded_channel();
        let mut position = 0u64;

        read_file_content(&file_path, &mut position, "\n", &tx)
            .await
            .expect("Should read file successfully");

        let messages = collect_messages(rx).await;

        // Should only get non-empty lines - one Vec<String> with 3 lines
        assert_eq!(messages.len(), 1);
        let lines = &messages[0];
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[1], "line2");
        assert_eq!(lines[2], "line3");

        // Clean up
        fs::remove_file(temp_file).await.unwrap();
    }

    #[tokio::test]
    async fn test_utf8_handling_valid_content() {
        let temp_file = "test_utf8_valid.tmp";
        let utf8_content = "Hello 世界\nUnicode: 🦀\n日本語テスト\n";
        fs::write(temp_file, utf8_content).await.unwrap();

        let file_path = PathBuf::from(temp_file);
        let (tx, rx) = mpsc::unbounded_channel();
        let mut position = 0u64;

        read_file_content(&file_path, &mut position, "\n", &tx)
            .await
            .expect("Should read UTF-8 content successfully");

        let messages = collect_messages(rx).await;

        assert_eq!(messages.len(), 1);
        let lines = &messages[0];
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "Hello 世界");
        assert_eq!(lines[1], "Unicode: 🦀");
        assert_eq!(lines[2], "日本語テスト");

        // Clean up
        fs::remove_file(temp_file).await.unwrap();
    }

    #[tokio::test]
    async fn test_large_file_reading() {
        let temp_file = "test_large_file.tmp";

        // Create a large file with many lines
        let mut large_content = String::new();
        for i in 0..1000 {
            large_content.push_str(&format!("Line number {}\n", i));
        }
        fs::write(temp_file, &large_content).await.unwrap();

        let file_path = PathBuf::from(temp_file);
        let (tx, rx) = mpsc::unbounded_channel();
        let mut position = 0u64;

        read_file_content(&file_path, &mut position, "\n", &tx)
            .await
            .expect("Should read large file successfully");

        let messages = collect_messages(rx).await;

        // Should get all 1000 lines in one Vec
        assert_eq!(messages.len(), 1);
        let lines = &messages[0];
        assert_eq!(lines.len(), 1000);
        assert_eq!(lines[0], "Line number 0");
        assert_eq!(lines[999], "Line number 999");

        // Clean up
        fs::remove_file(temp_file).await.unwrap();
    }

    #[tokio::test]
    async fn test_file_with_very_long_lines() {
        let temp_file = "test_long_lines.tmp";

        // Create a file with very long lines
        let long_line = "A".repeat(10000);
        let content = format!("{}\n{}\n", long_line, "short line");
        fs::write(temp_file, &content).await.unwrap();

        let file_path = PathBuf::from(temp_file);
        let (tx, rx) = mpsc::unbounded_channel();
        let mut position = 0u64;

        read_file_content(&file_path, &mut position, "\n", &tx)
            .await
            .expect("Should read file with long lines successfully");

        let messages = collect_messages(rx).await;

        assert_eq!(messages.len(), 1);
        let lines = &messages[0];
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].len(), 10000);
        assert!(lines[0].chars().all(|c| c == 'A'));
        assert_eq!(lines[1], "short line");

        // Clean up
        fs::remove_file(temp_file).await.unwrap();
    }

    #[tokio::test]
    async fn test_binary_like_content_handling() {
        let temp_file = "test_binary_content.tmp";

        // Create content with null bytes and other binary-like data
        let content = "line1\nline with \0 null byte\nline3\n";
        fs::write(temp_file, content).await.unwrap();

        let file_path = PathBuf::from(temp_file);
        let (tx, rx) = mpsc::unbounded_channel();
        let mut position = 0u64;

        read_file_content(&file_path, &mut position, "\n", &tx)
            .await
            .expect("Should read binary-like content successfully");

        let messages = collect_messages(rx).await;

        assert_eq!(messages.len(), 1);
        let lines = &messages[0];
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line1");
        assert!(lines[1].contains("null byte"));
        assert_eq!(lines[2], "line3");

        // Clean up
        fs::remove_file(temp_file).await.unwrap();
    }

    #[tokio::test]
    async fn test_concurrent_reading_attempts() {
        let file_path = PathBuf::from("fixtures/simple_append.log");

        // Spawn multiple concurrent reading tasks
        let mut handles = Vec::new();

        for i in 0..5 {
            let path = file_path.clone();
            let handle = tokio::spawn(async move {
                let (tx, rx) = mpsc::unbounded_channel();
                let mut position = 0u64;

                read_file_content(&path, &mut position, "\n", &tx)
                    .await
                    .expect("Should read file successfully");

                let messages = collect_messages(rx).await;
                (i, messages.len())
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results: Vec<_> = futures::future::join_all(handles).await;

        // All tasks should complete successfully
        for result in results {
            let (task_id, message_count) = result.unwrap();
            assert!(
                message_count > 0,
                "Task {} should have read some messages",
                task_id
            );
        }
    }

    #[test]
    fn test_split_and_filter_content_edge_cases() {
        // Test with separator at the beginning
        let content = "\nline1\nline2";
        let result = split_and_filter_content(content, "\n");
        assert_eq!(result, vec!["line1", "line2"]);

        // Test with separator at the end
        let content = "line1\nline2\n";
        let result = split_and_filter_content(content, "\n");
        assert_eq!(result, vec!["line1", "line2"]);

        // Test with repeated separators
        let content = "line1\n\n\n\nline2";
        let result = split_and_filter_content(content, "\n");
        assert_eq!(result, vec!["line1", "line2"]);

        // Test with whitespace-only content between separators
        let content = "line1\n   \n\t\n  \nline2";
        let result = split_and_filter_content(content, "\n");
        assert_eq!(result, vec!["line1", "line2"]);
    }

    #[test]
    fn test_position_calculation_edge_cases() {
        // Test with zero values
        assert_eq!(calculate_bytes_to_read(0, 0), None);

        // Test with large values
        assert_eq!(calculate_bytes_to_read(u64::MAX, u64::MAX - 1), Some(1));
        assert_eq!(calculate_bytes_to_read(u64::MAX - 1, u64::MAX), None);

        // Test boundary conditions
        assert_eq!(calculate_bytes_to_read(1, 0), Some(1));
        assert_eq!(calculate_bytes_to_read(0, 1), None);
    }

    #[test]
    fn test_file_truncation_edge_cases() {
        // Test with equal sizes
        assert!(!detect_file_truncation(100, 100));

        // Test with zero values
        assert!(!detect_file_truncation(0, 0));
        assert!(detect_file_truncation(0, 1));

        // Test with large values
        assert!(detect_file_truncation(u64::MAX - 1, u64::MAX));
        assert!(!detect_file_truncation(u64::MAX, u64::MAX - 1));
    }
}
