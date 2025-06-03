//! Error types for the log reader library.

use thiserror::Error;

/// The main error type for log reader operations.
#[derive(Error, Debug)]
pub enum Error {
    /// I/O errors when reading files or watching for changes.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// File watching errors from the notify crate.
    #[error("File watcher error: {0}")]
    Watcher(#[from] notify::Error),

    /// UTF-8 decoding errors when reading file content.
    #[error("UTF-8 decoding error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    /// File path errors.
    #[error("Invalid file path: {message}")]
    InvalidPath { message: String },

    /// File has been removed or is no longer accessible.
    #[error("File no longer exists: {path}")]
    FileNotFound { path: String },

    /// Stream has been closed or dropped.
    #[error("Stream closed")]
    StreamClosed,
}

/// A convenient Result type for log reader operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn test_io_error_conversion() {
        let io_error = IoError::new(ErrorKind::NotFound, "File not found");
        let error: Error = io_error.into();

        match error {
            Error::Io(_) => {}
            _ => panic!("Expected Error::Io variant"),
        }

        assert!(error.to_string().contains("I/O error"));
        assert!(error.to_string().contains("File not found"));
    }

    #[test]
    fn test_watcher_error_conversion() {
        let notify_error = notify::Error::generic("Test watcher error");
        let error: Error = notify_error.into();

        match error {
            Error::Watcher(_) => {}
            _ => panic!("Expected Error::Watcher variant"),
        }

        assert!(error.to_string().contains("File watcher error"));
        assert!(error.to_string().contains("Test watcher error"));
    }

    #[test]
    fn test_utf8_error_conversion() {
        let utf8_error = String::from_utf8(vec![0, 159, 146, 150]).unwrap_err();
        let error: Error = utf8_error.into();

        match error {
            Error::Utf8(_) => {}
            _ => panic!("Expected Error::Utf8 variant"),
        }

        assert!(error.to_string().contains("UTF-8 decoding error"));
    }

    #[test]
    fn test_invalid_path_error() {
        let error = Error::InvalidPath {
            message: "Path contains invalid characters".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "Invalid file path: Path contains invalid characters"
        );
    }

    #[test]
    fn test_file_not_found_error() {
        let error = Error::FileNotFound {
            path: "/path/to/missing/file.log".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "File no longer exists: /path/to/missing/file.log"
        );
    }

    #[test]
    fn test_stream_closed_error() {
        let error = Error::StreamClosed;
        assert_eq!(error.to_string(), "Stream closed");
    }

    #[test]
    fn test_error_debug_format() {
        let error = Error::StreamClosed;
        let debug_str = format!("{:?}", error);
        assert_eq!(debug_str, "StreamClosed");
    }

    #[test]
    fn test_result_type_alias() {
        let success: Result<i32> = Ok(42);
        let failure: Result<i32> = Err(Error::StreamClosed);

        assert!(success.is_ok());
        assert!(failure.is_err());
        assert_eq!(success.unwrap(), 42);

        match failure {
            Err(Error::StreamClosed) => {}
            _ => panic!("Expected StreamClosed error"),
        }
    }

    #[test]
    fn test_error_chain_with_io_error() {
        let io_error = IoError::new(ErrorKind::PermissionDenied, "Access denied");
        let error: Error = io_error.into();

        // Test that the original error is preserved in the chain
        match &error {
            Error::Io(inner) => {
                assert_eq!(inner.kind(), ErrorKind::PermissionDenied);
                assert_eq!(inner.to_string(), "Access denied");
            }
            _ => panic!("Expected Error::Io variant"),
        }
    }

    #[test]
    fn test_error_send_sync_traits() {
        // Ensure our error type implements Send + Sync for async compatibility
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<Error>();
        assert_sync::<Error>();
    }
}
