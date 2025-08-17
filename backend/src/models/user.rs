//! User data model.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Application user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct User {
    /// Stable user identifier
    #[schema(example = "u_1")]
    pub id: String,
    /// Display name shown to other users
    #[schema(example = "Ada")]
    pub display_name: String,
}
