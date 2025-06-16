# log-reader

A Rust library that provides real-time streaming of file contents, monitoring files for changes and emitting new content as an async stream.

## Features

- **Batch Reading**: Reads entire file contents and emits them as `Vec<String>` for efficient processing
- **Incremental Updates**: When files change, only new content is read and emitted
- **Real-time Monitoring**: Uses file system watching for immediate updates
- **Custom Separators**: Support for custom line separators (defaults to newline)
- **Position Tracking**: Handles file truncation and maintains read position across changes

## Usage

### `watch_log(path, separator)`

Creates a stream that watches a file for new content.

- `path` - File path to monitor
- `separator` - Content separator (defaults to newline)

Returns a `Stream` of `Vec<String>` containing lines from the file.

## Example

```rust
use log_reader::watch_log;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = watch_log("app.log", None).await?;
    
    while let Some(lines_result) = stream.next().await {
        match lines_result {
            Ok(lines) => {
                println!("Received {} lines:", lines.len());
                for (i, line) in lines.iter().enumerate() {
                    println!("  [{}]: {}", i + 1, line);
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    
    Ok(())
}
```

## Behavior

- **Initial Read**: When first watching a file, the entire existing content is read and emitted as one `Vec<String>`
- **Incremental Updates**: When the file is modified, only the new content (from last position to end of file) is emitted as a `Vec<String>`
- **Empty Results**: If there are no new lines to emit, no message is sent through the stream
- **File Truncation**: Automatically detects and handles file truncation (e.g., log rotation)
