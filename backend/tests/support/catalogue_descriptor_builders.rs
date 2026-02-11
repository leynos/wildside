//! Shared builders and identifiers for catalogue/descriptor ingestion tests.

use std::collections::BTreeMap;

use backend::domain::{ImageAsset, LocalizationMap, LocalizedStringSet, SemanticIconIdentifier};
use uuid::Uuid;

pub(crate) const ROUTE_CATEGORY_ID: Uuid = Uuid::from_u128(0x11111111111111111111111111111111);
pub(crate) const THEME_ID: Uuid = Uuid::from_u128(0x22222222222222222222222222222222);
pub(crate) const ROUTE_COLLECTION_ID: Uuid = Uuid::from_u128(0x33333333333333333333333333333333);
pub(crate) const ROUTE_SUMMARY_ID: Uuid = Uuid::from_u128(0x44444444444444444444444444444444);
pub(crate) const HIGHLIGHT_ID: Uuid = Uuid::from_u128(0x55555555555555555555555555555555);
pub(crate) const COMMUNITY_PICK_ID: Uuid = Uuid::from_u128(0x66666666666666666666666666666666);
pub(crate) const EDGE_COMMUNITY_PICK_ID: Uuid = Uuid::from_u128(0x77777777777777777777777777777777);
pub(crate) const TAG_ID: Uuid = Uuid::from_u128(0x88888888888888888888888888888888);
pub(crate) const BADGE_ID: Uuid = Uuid::from_u128(0x99999999999999999999999999999999);
pub(crate) const SAFETY_TOGGLE_ID: Uuid = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
pub(crate) const SAFETY_PRESET_ID: Uuid = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
pub(crate) const ROUTE_ID: Uuid = Uuid::from_u128(0xcccccccccccccccccccccccccccccccc);
pub(crate) const CURATOR_USER_ID: Uuid = Uuid::from_u128(0xdddddddddddddddddddddddddddddddd);

pub(crate) fn icon(value: &str) -> SemanticIconIdentifier {
    SemanticIconIdentifier::new(value).expect("icon identifier fixture should be valid")
}

pub(crate) fn localizations(name: &str) -> LocalizationMap {
    let mut values = BTreeMap::new();
    values.insert(
        "en-GB".to_owned(),
        LocalizedStringSet::new(
            name.to_owned(),
            Some(format!("{name} short")),
            Some(format!("{name} description")),
        ),
    );
    values.insert(
        "fr-FR".to_owned(),
        LocalizedStringSet::new(
            format!("{name} FR"),
            Some(format!("{name} FR court")),
            Some(format!("{name} FR description")),
        ),
    );
    LocalizationMap::new(values).expect("localization fixture should be valid")
}

pub(crate) fn image(url: &str, alt: &str) -> ImageAsset {
    ImageAsset::new(url, alt).expect("image fixture should be valid")
}
