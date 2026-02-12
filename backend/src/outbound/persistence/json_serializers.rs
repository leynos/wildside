//! Shared JSON serialization helpers for outbound Diesel adapters.

use serde_json::{Map, Value};

use crate::domain::{ImageAsset, LocalizationMap, LocalizedStringSet};

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
