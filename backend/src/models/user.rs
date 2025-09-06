//! User data model.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Application user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct User {
    /// Stable user identifier
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub id: String,
    /// Display name shown to other users
    #[schema(example = "Ada Lovelace")]
    pub display_name: String,
}
