//! User data model.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Application user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct User {
    /// Stable user identifier
    #[schema(example = "00000000-0000-0000-0000-000000000000")]
    pub id: Uuid,
    /// Display name shown to other users
    #[schema(example = "Ada")]
    pub display_name: String,
}
