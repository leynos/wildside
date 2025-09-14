//! User data model.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Application user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct User {
    /// Stable user identifier
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub id: String,
    /// Display name shown to other users
    #[schema(example = "Ada Lovelace")]
    #[serde(alias = "display_name")]
    pub display_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_display_name_alias() {
        let camel: User = serde_json::from_value(json!({"id":"1","displayName":"Alice"}))
            .expect("camelCase should deserialize");
        let snake: User = serde_json::from_value(json!({"id":"1","display_name":"Alice"}))
            .expect("snake_case should deserialize");
        assert_eq!(camel, snake);
        let value = serde_json::to_value(snake).expect("serialize to JSON");
        assert_eq!(
            value.get("displayName").and_then(|v| v.as_str()),
            Some("Alice")
        );
        assert!(value.get("display_name").is_none());
    }
}
