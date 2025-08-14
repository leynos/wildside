//! User data model.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Application user.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct User {
    /// Stable user identifier
    pub id: String,
    /// Display name shown to other users
    pub display_name: String,
}
