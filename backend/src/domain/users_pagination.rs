//! User pagination ordering keys.
//!
//! The users list is ordered by creation time and then identifier. This module
//! keeps that ordering key in the domain so inbound handlers and outbound
//! persistence adapters can share cursor semantics without coupling to each
//! other.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::User;

/// Stable key encoded into user-list pagination cursors.
///
/// # Examples
///
/// ```
/// use backend::domain::UserCursorKey;
/// use chrono::{DateTime, Utc};
/// use pagination::{Cursor, Direction};
/// use uuid::Uuid;
///
/// let created_at = DateTime::parse_from_rfc3339("2026-05-01T12:00:00Z")
///     .expect("valid timestamp")
///     .with_timezone(&Utc);
/// let id = Uuid::parse_str("11111111-1111-1111-1111-111111111111")
///     .expect("valid UUID");
/// let key = UserCursorKey::new(created_at, id);
///
/// let encoded = Cursor::new(key.clone()).encode().expect("encode cursor");
/// let decoded = Cursor::<UserCursorKey>::decode(&encoded).expect("decode cursor");
///
/// assert_eq!(decoded.key(), &key);
/// assert_eq!(decoded.direction(), Direction::Next);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCursorKey {
    /// Creation timestamp for the row at the cursor boundary.
    pub created_at: DateTime<Utc>,
    /// User identifier used to break timestamp ties deterministically.
    pub id: Uuid,
}

impl UserCursorKey {
    /// Build a user cursor key from explicit ordering components.
    #[must_use]
    pub const fn new(created_at: DateTime<Utc>, id: Uuid) -> Self {
        Self { created_at, id }
    }
}

impl From<&User> for UserCursorKey {
    fn from(value: &User) -> Self {
        Self::new(value.created_at(), *value.id().as_uuid())
    }
}
