//! Test utilities for session configuration.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "helper functions used only in tests; allowed when built non-test to avoid unreachable warnings"
    )
)]

use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

// Ensure the dead_code expectation is exercised in non-test builds.
#[cfg(not(test))]
const TEST_UTILS_LINT_GUARD: () = ();

/// A temporary session key file for testing purposes.
///
/// It creates a file in the system temporary directory filled with dummy data,
/// and the file is automatically removed when the instance is dropped.
#[derive(Debug)]
pub struct TempKeyFile {
    path: PathBuf,
}

impl TempKeyFile {
    /// Creates a temporary session key file filled with dummy data.
    ///
    /// # Parameters
    ///
    /// - `len`: The number of bytes to write to the temporary file.
    ///
    /// # Returns
    ///
    /// Returns `Ok(TempKeyFile)` on success, or a `std::io::Error` on failure.
    /// The file is created in the system temporary directory with a UUID-based
    /// name.
    ///
    /// # Errors
    ///
    /// Returns an IO error if the file cannot be created or written.
    pub fn new(len: usize) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("session-key-{}", Uuid::new_v4()));
        let mut file = std::fs::File::create(&path)?;
        let buffer = [b'a'; 4096];
        let mut remaining = len;
        while remaining > 0 {
            let chunk_len = remaining.min(buffer.len());
            file.write_all(&buffer[..chunk_len])?;
            remaining -= chunk_len;
        }
        Ok(Self { path })
    }

    /// Returns the file path as a `String`, replacing non-UTF-8 sequences with
    /// the Unicode replacement character via `to_string_lossy`.
    pub fn path_str(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
}

impl Drop for TempKeyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
