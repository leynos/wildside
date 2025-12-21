//! Test utilities for session configuration.

#![cfg_attr(not(test), allow(dead_code))]

use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug)]
pub struct TempKeyFile {
    path: PathBuf,
}

impl TempKeyFile {
    pub fn new(len: usize) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("session-key-{}", Uuid::new_v4()));
        std::fs::write(&path, vec![b'a'; len])?;
        Ok(Self { path })
    }

    pub fn path_str(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
}

impl Drop for TempKeyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
