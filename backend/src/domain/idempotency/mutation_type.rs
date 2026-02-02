//! Mutation type discriminators for idempotent operations.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The type of mutation protected by idempotency.
///
/// Each variant corresponds to an outbox-backed operation that supports
/// idempotent retries. The discriminator ensures keys are isolated per
/// mutation kind, preventing collisions when different operations use
/// the same UUID.
///
/// # Example
///
/// ```
/// # use backend::domain::idempotency::MutationType;
/// let mutation = MutationType::Routes;
/// assert_eq!(mutation.as_str(), "routes");
/// assert_eq!(mutation.to_string(), "routes");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationType {
    /// Route submission (`POST /api/v1/routes`).
    Routes,
    /// Route note upsert (`POST /api/v1/routes/{route_id}/notes`).
    Notes,
    /// Route progress update (`PUT /api/v1/routes/{route_id}/progress`).
    Progress,
    /// User preferences update (`PUT /api/v1/users/me/preferences`).
    Preferences,
    /// Offline bundle operations (`POST/DELETE /api/v1/offline/bundles`).
    Bundles,
}

impl MutationType {
    /// All mutation type variants.
    ///
    /// Useful for iteration, validation, and documentation.
    pub const ALL: [MutationType; 5] = [
        MutationType::Routes,
        MutationType::Notes,
        MutationType::Progress,
        MutationType::Preferences,
        MutationType::Bundles,
    ];
}

impl MutationType {
    /// Returns the database string representation.
    ///
    /// # Example
    ///
    /// ```
    /// # use backend::domain::idempotency::MutationType;
    /// assert_eq!(MutationType::Routes.as_str(), "routes");
    /// assert_eq!(MutationType::Notes.as_str(), "notes");
    /// assert_eq!(MutationType::Progress.as_str(), "progress");
    /// assert_eq!(MutationType::Preferences.as_str(), "preferences");
    /// assert_eq!(MutationType::Bundles.as_str(), "bundles");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Routes => "routes",
            Self::Notes => "notes",
            Self::Progress => "progress",
            Self::Preferences => "preferences",
            Self::Bundles => "bundles",
        }
    }
}

impl fmt::Display for MutationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when parsing an invalid mutation type string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseMutationTypeError {
    /// The invalid input string.
    pub input: String,
}

impl fmt::Display for ParseMutationTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variants: Vec<_> = MutationType::ALL.iter().map(|v| v.as_str()).collect();
        write!(
            f,
            "invalid mutation type '{}': expected one of {}",
            self.input,
            variants.join(", ")
        )
    }
}

impl std::error::Error for ParseMutationTypeError {}

impl FromStr for MutationType {
    type Err = ParseMutationTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .find(|v| v.as_str() == s)
            .copied()
            .ok_or_else(|| ParseMutationTypeError {
                input: s.to_owned(),
            })
    }
}
