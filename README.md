# log-reader

A Rust library that provides real-time streaming of file contents, monitoring files for changes and emitting new content as an async stream.

## Usage

### `watch_log(path, separator)`

Creates a stream that watches a file for new content.

- `path` - File path to monitor
- `separator` - Content separator (defaults to newline)

Returns a `Stream` of new content from the file.


## Example

```rust
use log_reader::watch_log;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = watch_log("app.log", None).await?;
    
    while let Some(line) = stream.next().await {
        println!("New content: {}", line?);
    }
    
    Ok(())
}
```
