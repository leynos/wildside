//! User data model.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Application user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct User {
    /// Stable user identifier
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: String,
    /// Display name shown to other users
    #[schema(example = "Ada Lovelace")]
    pub display_name: String,
}
