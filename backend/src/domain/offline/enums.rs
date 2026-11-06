//! Offline bundle enum types and parsers.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Bundle scope in offline storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfflineBundleKind {
    /// Bundle covering a region rather than a single route.
    Region,
    /// Bundle covering a single route.
    Route,
}

impl OfflineBundleKind {
    /// Returns the snake_case name used in serialized form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Route => "route",
        }
    }
}

impl fmt::Display for OfflineBundleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parse error for [`OfflineBundleKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOfflineBundleKindError {
    /// Text that did not match a known kind.
    pub input: String,
}

impl fmt::Display for ParseOfflineBundleKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid offline bundle kind: {}", self.input)
    }
}

impl std::error::Error for ParseOfflineBundleKindError {}

impl FromStr for OfflineBundleKind {
    type Err = ParseOfflineBundleKindError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "region" => Ok(Self::Region),
            "route" => Ok(Self::Route),
            _ => Err(ParseOfflineBundleKindError {
                input: value.to_owned(),
            }),
        }
    }
}

/// Lifecycle state for offline bundle downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfflineBundleStatus {
    /// Accepted and waiting to start; progress is zero.
    Queued,
    /// Tiles are being fetched; progress is below 1.0.
    Downloading,
    /// All tiles are downloaded; progress is exactly 1.0.
    Complete,
    /// The download ended in an error.
    Failed,
}

impl OfflineBundleStatus {
    /// Returns the lowercase name used in serialized form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for OfflineBundleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parse error for [`OfflineBundleStatus`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOfflineBundleStatusError {
    /// Text that did not match a known status.
    pub input: String,
}

impl fmt::Display for ParseOfflineBundleStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid offline bundle status: {}", self.input)
    }
}

impl std::error::Error for ParseOfflineBundleStatusError {}

impl FromStr for OfflineBundleStatus {
    type Err = ParseOfflineBundleStatusError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "downloading" => Ok(Self::Downloading),
            "complete" => Ok(Self::Complete),
            "failed" => Ok(Self::Failed),
            _ => Err(ParseOfflineBundleStatusError {
                input: value.to_owned(),
            }),
        }
    }
}
