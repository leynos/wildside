//! Shared JSON serialization helpers for outbound Diesel adapters.
//!
//! Encode helpers convert domain types to `serde_json::Value` for JSONB
//! persistence.  Decode helpers reverse this for read-side adapters,
//! validating through domain constructors so malformed payloads surface as
//! typed errors rather than silent data corruption.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::domain::{
    ImageAsset, LocaleCode, LocalizationMap, LocalizedStringSet, SemanticIconIdentifier,
};

pub(super) fn localized_string_set_to_json(localized: &LocalizedStringSet) -> Value {
    let mut object = Map::with_capacity(3);
    object.insert("name".to_owned(), Value::String(localized.name.clone()));
    if let Some(short_label) = &localized.short_label {
        object.insert("shortLabel".to_owned(), Value::String(short_label.clone()));
    }
    if let Some(description) = &localized.description {
        object.insert("description".to_owned(), Value::String(description.clone()));
    }
    Value::Object(object)
}

pub(super) fn localization_map_to_json(localizations: &LocalizationMap) -> Value {
    let values = localizations
        .as_map()
        .iter()
        .map(|(locale, set)| (locale.clone(), localized_string_set_to_json(set)))
        .collect::<Map<_, _>>();
    Value::Object(values)
}

pub(super) fn image_asset_to_json(image: &ImageAsset) -> Value {
    Value::Object(Map::from_iter([
        ("url".to_owned(), Value::String(image.url.clone())),
        ("alt".to_owned(), Value::String(image.alt.clone())),
    ]))
}

// ---------------------------------------------------------------------------
// Decode helpers (JSONB → domain)
// ---------------------------------------------------------------------------

/// Decode a JSONB localization map into a validated [`LocalizationMap`].
///
/// Accepts the [`Value`] by value so `serde_json::from_value` can consume it
/// directly, avoiding an unnecessary clone.
///
/// # Examples
///
/// ```rust,ignore
/// let json = serde_json::json!({"en-GB": {"name": "Scenic"}});
/// let map = json_to_localization_map(json).unwrap();
/// assert!(map.as_map().contains_key("en-GB"));
/// ```
pub(super) fn json_to_localization_map(value: Value) -> Result<LocalizationMap, String> {
    let raw: BTreeMap<LocaleCode, LocalizedStringSet> =
        serde_json::from_value(value).map_err(|e| format!("localization decode: {e}"))?;
    LocalizationMap::new(raw).map_err(|e| format!("localization validation: {e}"))
}

/// Decode a JSONB image asset into a validated [`ImageAsset`].
///
/// # Examples
///
/// ```rust,ignore
/// let json = serde_json::json!({"url": "https://example.com/img.jpg", "alt": "A photo"});
/// let asset = json_to_image_asset(&json).unwrap();
/// assert_eq!(asset.url, "https://example.com/img.jpg");
/// ```
pub(super) fn json_to_image_asset(value: &Value) -> Result<ImageAsset, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "image asset: expected JSON object".to_owned())?;
    let url = obj
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "image asset: missing or non-string 'url'".to_owned())?;
    let alt = obj
        .get("alt")
        .and_then(Value::as_str)
        .ok_or_else(|| "image asset: missing or non-string 'alt'".to_owned())?;
    ImageAsset::new(url, alt).map_err(|e| format!("image asset validation: {e}"))
}

/// Decode a raw icon key string into a validated [`SemanticIconIdentifier`].
///
/// # Examples
///
/// ```rust,ignore
/// let icon = json_to_semantic_icon_identifier("category:scenic").unwrap();
/// assert_eq!(icon.to_string(), "category:scenic");
/// ```
pub(super) fn json_to_semantic_icon_identifier(
    raw: &str,
) -> Result<SemanticIconIdentifier, String> {
    SemanticIconIdentifier::new(raw).map_err(|e| format!("icon identifier validation: {e}"))
}

#[cfg(test)]
mod tests {
    //! Unit tests for JSON decode helpers.

    use super::*;
    use rstest::{fixture, rstest};
    use serde_json::json;

    #[fixture]
    fn sample_localization_map() -> LocalizationMap {
        let mut values = BTreeMap::new();
        values.insert(
            "en-GB".to_owned(),
            LocalizedStringSet::new("Nature walk", Some("Nature".to_owned()), None),
        );
        values.insert(
            "fr-FR".to_owned(),
            LocalizedStringSet::new(
                "Promenade nature",
                None,
                Some("Une balade en pleine nature".to_owned()),
            ),
        );
        LocalizationMap::new(values).expect("fixture should be valid")
    }

    #[fixture]
    fn sample_image_asset() -> ImageAsset {
        ImageAsset::new("https://example.test/hero.jpg", "Route hero")
            .expect("fixture should be valid")
    }

    // -- localization round-trip --

    #[rstest]
    fn localization_map_round_trips_through_json(sample_localization_map: LocalizationMap) {
        let json = localization_map_to_json(&sample_localization_map);
        let decoded = json_to_localization_map(json).expect("decode should succeed");
        assert_eq!(decoded, sample_localization_map);
    }

    #[rstest]
    fn localization_map_rejects_empty_json_object() {
        let json = json!({});
        let err = json_to_localization_map(json).expect_err("empty map should fail");
        assert!(
            err.contains("localization validation"),
            "unexpected error: {err}"
        );
    }

    #[rstest]
    fn localization_map_rejects_missing_name_field() {
        let json = json!({ "en-GB": { "shortLabel": "Short" } });
        let err = json_to_localization_map(json).expect_err("missing name should fail");
        assert!(
            err.contains("localization decode"),
            "unexpected error: {err}"
        );
    }

    // -- image asset round-trip --

    #[rstest]
    fn image_asset_round_trips_through_json(sample_image_asset: ImageAsset) {
        let json = image_asset_to_json(&sample_image_asset);
        let decoded = json_to_image_asset(&json).expect("decode should succeed");
        assert_eq!(decoded, sample_image_asset);
    }

    #[rstest]
    fn image_asset_rejects_missing_url() {
        let json = json!({ "alt": "Hero" });
        let err = json_to_image_asset(&json).expect_err("missing url should fail");
        assert!(err.contains("url"), "unexpected error: {err}");
    }

    #[rstest]
    fn image_asset_rejects_non_object() {
        let json = json!("not an object");
        let err = json_to_image_asset(&json).expect_err("non-object should fail");
        assert!(
            err.contains("expected JSON object"),
            "unexpected error: {err}"
        );
    }

    // -- semantic icon identifier --

    #[rstest]
    fn semantic_icon_identifier_accepts_valid_key() {
        let result = json_to_semantic_icon_identifier("category:nature");
        assert!(result.is_ok());
        assert_eq!(result.expect("valid").as_ref(), "category:nature");
    }

    #[rstest]
    fn semantic_icon_identifier_rejects_empty_string() {
        let err = json_to_semantic_icon_identifier("").expect_err("empty should fail");
        assert!(
            err.contains("icon identifier validation"),
            "unexpected error: {err}"
        );
    }
}
