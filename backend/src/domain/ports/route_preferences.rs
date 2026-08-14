//! Bounded route-generation preference values.

use std::fmt;

use serde::de::{Error as DeError, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

/// Maximum number of values accepted in one route-preference list.
pub const ROUTE_PREFERENCE_MAX_ITEMS: usize = 64;
/// Maximum UTF-8 byte length of one route-preference value.
pub const ROUTE_PREFERENCE_MAX_VALUE_BYTES: usize = 64;

/// Optional route-generation preferences supported by the HTTP contract.
///
/// String values and list entries are bounded during deserialization so the
/// same limits apply to HTTP requests and persisted job payloads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutePreferences {
    /// Routing mode, such as `walking`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_preference_value"
    )]
    #[schema(max_length = 64)]
    pub mode: Option<String>,
    /// Theme names used to bias route generation.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_preference_values"
    )]
    #[schema(max_items = 64)]
    pub themes: Option<Vec<String>>,
    /// Theme identifiers used to bias route generation.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_preference_values"
    )]
    #[schema(max_items = 64)]
    pub theme_ids: Option<Vec<String>>,
    /// Interest-theme identifiers used to bias route generation.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_preference_values"
    )]
    #[schema(max_items = 64)]
    pub interest_theme_ids: Option<Vec<String>>,
    /// Route features that should be avoided.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_preference_values"
    )]
    #[schema(max_items = 64)]
    pub avoid: Option<Vec<String>>,
    /// Whether routes should avoid stairs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avoid_stairs: Option<bool>,
}

fn deserialize_optional_preference_value<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_option(OptionalPreferenceValueVisitor)
}

fn deserialize_optional_preference_values<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_option(OptionalPreferenceValuesVisitor)
}

struct OptionalPreferenceValueVisitor;

impl<'de> Visitor<'de> for OptionalPreferenceValueVisitor {
    type Value = Option<String>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a route preference value or null")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        BoundedPreferenceValue::deserialize(deserializer).map(|value| Some(value.0))
    }
}

struct OptionalPreferenceValuesVisitor;

impl<'de> Visitor<'de> for OptionalPreferenceValuesVisitor {
    type Value = Option<Vec<String>>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a route preference list or null")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer
            .deserialize_seq(PreferenceValuesVisitor)
            .map(Some)
    }
}

struct PreferenceValuesVisitor;

impl<'de> Visitor<'de> for PreferenceValuesVisitor {
    type Value = Vec<String>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "at most {ROUTE_PREFERENCE_MAX_ITEMS} route preference values"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = sequence
            .size_hint()
            .unwrap_or_default()
            .min(ROUTE_PREFERENCE_MAX_ITEMS);
        let mut values = Vec::with_capacity(capacity);

        while values.len() < ROUTE_PREFERENCE_MAX_ITEMS {
            let Some(value) = sequence.next_element::<BoundedPreferenceValue>()? else {
                return Ok(values);
            };
            values.push(value.0);
        }

        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(format!(
                "route preference list exceeds {ROUTE_PREFERENCE_MAX_ITEMS} items"
            )));
        }

        Ok(values)
    }
}

struct BoundedPreferenceValue(String);

impl<'de> Deserialize<'de> for BoundedPreferenceValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(BoundedPreferenceValueVisitor)
    }
}

struct BoundedPreferenceValueVisitor;

impl<'de> Visitor<'de> for BoundedPreferenceValueVisitor {
    type Value = BoundedPreferenceValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a route preference value no longer than \
             {ROUTE_PREFERENCE_MAX_VALUE_BYTES} UTF-8 bytes"
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        validate_preference_value(value).map_err(E::custom)?;
        Ok(BoundedPreferenceValue(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        validate_preference_value(&value).map_err(E::custom)?;
        Ok(BoundedPreferenceValue(value))
    }
}

fn validate_preference_value(value: &str) -> Result<(), String> {
    if value.len() > ROUTE_PREFERENCE_MAX_VALUE_BYTES {
        return Err(format!(
            "route preference value exceeds {ROUTE_PREFERENCE_MAX_VALUE_BYTES} UTF-8 bytes"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Regression coverage for route-preference deserialization limits.

    use super::*;
    use serde_json::json;

    #[test]
    fn deserialization_accepts_preference_boundaries() {
        let maximum_value = "x".repeat(ROUTE_PREFERENCE_MAX_VALUE_BYTES);
        let maximum_values = vec![maximum_value.clone(); ROUTE_PREFERENCE_MAX_ITEMS];
        let value = json!({
            "mode": maximum_value.clone(),
            "themes": maximum_values,
        });

        let preferences: RoutePreferences =
            serde_json::from_value(value).expect("bounded preferences should deserialize");

        assert_eq!(preferences.mode.as_deref(), Some(maximum_value.as_str()));
        assert_eq!(
            preferences.themes.as_ref().map(Vec::len),
            Some(ROUTE_PREFERENCE_MAX_ITEMS)
        );
    }

    #[test]
    fn deserialization_rejects_overlong_preference_values() {
        let value = json!({
            "mode": "é".repeat((ROUTE_PREFERENCE_MAX_VALUE_BYTES / 2) + 1),
        });

        let error = serde_json::from_value::<RoutePreferences>(value)
            .expect_err("overlong preference values should be rejected");

        assert!(error.to_string().contains("route preference value exceeds"));
    }

    #[test]
    fn deserialization_rejects_excess_preference_values() {
        let value = json!({
            "themes": vec!["heritage"; ROUTE_PREFERENCE_MAX_ITEMS + 1],
        });

        let error = serde_json::from_value::<RoutePreferences>(value)
            .expect_err("excess preference values should be rejected");

        assert!(error.to_string().contains("route preference list exceeds"));
    }
}
