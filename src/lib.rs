//! A log reader library that provides real-time streaming of file contents.
//! 
//! This library monitors files for changes and emits new content as an async stream,
//! with handling of file appends by tracking read positions.
//!
//! # Example
//!
//! ```rust,no_run
//! use log_reader::watch_log;
//! use tokio_stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut stream = watch_log("app.log", None).await?;
//!     
//!     while let Some(line) = stream.next().await {
//!         match line {
//!             Ok(content) => println!("New content: {}", content),
//!             Err(e) => eprintln!("Error: {}", e),
//!         }
//!     }
//!     
//!     Ok(())
//! }
//! ```

// Internal modules - not part of public API
mod error;
mod reader;
mod watcher;
mod stream;

#[cfg(test)]
mod test_helpers;

// Public API exports
pub use error::{Error, Result};
pub use stream::LogStream;

use std::path::Path;
use tokio_stream::Stream;

/// Creates a stream that watches a file for new content.
/// 
/// # Arguments
/// 
/// * `path` - File path to monitor
/// * `separator` - Content separator (defaults to newline)
/// 
/// # Example
/// 
/// ```rust,no_run
/// use log_reader::watch_log;
/// use tokio_stream::StreamExt;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut stream = watch_log("app.log", None).await?;
///     
///     while let Some(line) = stream.next().await {
///         println!("New content: {}", line?);
///     }
///     
///     Ok(())
/// }
/// ```
pub async fn watch_log<P: AsRef<Path>>(
    path: P,
    separator: Option<String>,
) -> Result<impl Stream<Item = Result<String>>> {
    LogStream::new(path, separator).await
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_basic_functionality() {
        // Placeholder test - will be implemented with proper fixtures
        assert!(true);
    }
}
