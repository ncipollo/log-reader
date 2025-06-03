//! Test utilities for creating temporary log files and collecting stream results.

#[cfg(test)]
use std::fs::{File, OpenOptions};
#[cfg(test)]
use std::io::Write;
#[cfg(test)]
use std::path::{Path, PathBuf};

#[cfg(test)]
pub struct TempLogFile {
    pub path: PathBuf,
    _temp_dir: tempfile::TempDir,
}

#[cfg(test)]
impl TempLogFile {
    /// Create a new temporary log file for testing
    pub fn new() -> std::io::Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().join("test.log");

        // Create the file
        File::create(&path)?;

        Ok(Self {
            path,
            _temp_dir: temp_dir,
        })
    }

    /// Create a temporary log file with initial content
    pub fn with_content(content: &str) -> std::io::Result<Self> {
        let temp_file = Self::new()?;
        temp_file.append_content(content)?;
        Ok(temp_file)
    }

    /// Append content to the temporary log file
    pub fn append_content(&self, content: &str) -> std::io::Result<()> {
        let mut file = OpenOptions::new().append(true).open(&self.path)?;

        writeln!(file, "{}", content)?;
        file.flush()?;
        Ok(())
    }

    /// Truncate the file (simulate log rotation)
    pub fn truncate(&self) -> std::io::Result<()> {
        File::create(&self.path)?;
        Ok(())
    }

    /// Get the path to the temporary file
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_temp_log_file_creation() {
        let temp_file = TempLogFile::new().unwrap();
        assert!(temp_file.path().exists());
    }

    #[tokio::test]
    async fn test_temp_log_file_with_content() {
        let content = "test line";
        let temp_file = TempLogFile::with_content(content).unwrap();

        let file_content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(file_content.contains(content));
    }

    #[tokio::test]
    async fn test_append_content() {
        let temp_file = TempLogFile::new().unwrap();
        temp_file.append_content("line 1").unwrap();
        temp_file.append_content("line 2").unwrap();

        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("line 1"));
        assert!(content.contains("line 2"));
    }

    #[tokio::test]
    async fn test_truncate() {
        let temp_file = TempLogFile::with_content("initial content").unwrap();
        temp_file.truncate().unwrap();

        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.is_empty());
    }
}
