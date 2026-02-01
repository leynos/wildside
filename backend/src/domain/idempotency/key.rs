//! Idempotency key validation and storage.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Validation errors for [`IdempotencyKey`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdempotencyKeyValidationError {
    /// The key string was empty.
    EmptyKey,
    /// The key string was not a valid UUID.
    InvalidKey,
}

impl fmt::Display for IdempotencyKeyValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => write!(f, "idempotency key must not be empty"),
            Self::InvalidKey => write!(f, "idempotency key must be a valid UUID"),
        }
    }
}

impl std::error::Error for IdempotencyKeyValidationError {}

/// Client-provided idempotency key (UUID v4).
///
/// Clients send this via the `Idempotency-Key` HTTP header to enable safe
/// request retries. The server uses the key to detect duplicate requests and
/// replay previously computed responses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdempotencyKey(Uuid, String);

impl IdempotencyKey {
    /// Validate and construct an [`IdempotencyKey`] from a string.
    ///
    /// # Errors
    ///
    /// Returns [`IdempotencyKeyValidationError::EmptyKey`] if the input is
    /// empty, or [`IdempotencyKeyValidationError::InvalidKey`] if the input is
    /// not a valid UUID.
    ///
    /// # Example
    ///
    /// ```
    /// # use backend::domain::idempotency::IdempotencyKey;
    /// let key = IdempotencyKey::new("550e8400-e29b-41d4-a716-446655440000")
    ///     .expect("valid UUID");
    /// assert_eq!(key.as_ref(), "550e8400-e29b-41d4-a716-446655440000");
    /// ```
    pub fn new(key: impl AsRef<str>) -> Result<Self, IdempotencyKeyValidationError> {
        Self::from_owned(key.as_ref().to_owned())
    }

    /// Construct an [`IdempotencyKey`] directly from a UUID.
    ///
    /// Useful when the UUID is already validated (e.g., loaded from database).
    pub fn from_uuid(uuid: Uuid) -> Self {
        let raw = uuid.to_string();
        Self(uuid, raw)
    }

    /// Generate a new random [`IdempotencyKey`].
    ///
    /// Primarily useful for testing.
    pub fn random() -> Self {
        let uuid = Uuid::new_v4();
        Self(uuid, uuid.to_string())
    }

    fn from_owned(key: String) -> Result<Self, IdempotencyKeyValidationError> {
        if key.is_empty() {
            return Err(IdempotencyKeyValidationError::EmptyKey);
        }
        if key.trim() != key {
            return Err(IdempotencyKeyValidationError::InvalidKey);
        }
        let parsed =
            Uuid::parse_str(&key).map_err(|_| IdempotencyKeyValidationError::InvalidKey)?;
        Ok(Self(parsed, key))
    }

    /// Access the underlying UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl AsRef<str> for IdempotencyKey {
    fn as_ref(&self) -> &str {
        self.1.as_str()
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl From<IdempotencyKey> for String {
    fn from(value: IdempotencyKey) -> Self {
        value.1
    }
}

impl TryFrom<String> for IdempotencyKey {
    type Error = IdempotencyKeyValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_owned(value)
    }
}
