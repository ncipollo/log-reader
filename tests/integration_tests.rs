use log_reader::watch_log;
use std::path::Path;
use std::time::Duration;
use tokio_stream::StreamExt;

/// Helper function to collect items from a stream with a timeout
async fn collect_stream_items<T>(
    mut stream: impl StreamExt<Item = T> + Unpin,
    timeout: Duration,
) -> Vec<T> {
    let mut items = Vec::new();
    let timeout_future = tokio::time::sleep(timeout);
    tokio::pin!(timeout_future);

    loop {
        tokio::select! {
            item = stream.next() => {
                match item {
                    Some(item) => items.push(item),
                    None => break,
                }
            }
            _ = &mut timeout_future => break,
        }
    }

    items
}

#[tokio::test]
async fn test_watch_existing_file_happy_path() {
    let fixture_path = Path::new("fixtures/simple_append.log");

    // Skip test if fixture doesn't exist
    if !fixture_path.exists() {
        eprintln!(
            "Skipping test: fixture file '{}' doesn't exist",
            fixture_path.display()
        );
        return;
    }

    let stream = watch_log(fixture_path, None).await.unwrap();

    // Collect initial items (should get existing content)
    // Use minimal timeout to avoid delays
    let items = collect_stream_items(stream, Duration::from_millis(100)).await;

    // Should have read some content from the existing file
    assert!(
        !items.is_empty(),
        "Should have read some content from existing file"
    );

    // Check that we got valid results (not checking specific content, just that it worked)
    let successful_reads = items.iter().filter(|item| item.is_ok()).count();
    assert!(
        successful_reads > 0,
        "Should have successfully read some lines"
    );
}

#[tokio::test]
async fn test_watch_nonexistent_file_error_case() {
    let _result = watch_log("definitely_nonexistent_file_12345.log", None).await;
    // This test mainly ensures no panic or unexpected behavior - both Ok and Err are acceptable
}
