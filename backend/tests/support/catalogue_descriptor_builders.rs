//! Shared builders and identifiers for catalogue/descriptor ingestion tests.

use std::collections::BTreeMap;

use backend::domain::{ImageAsset, LocalizationMap, LocalizedStringSet, SemanticIconIdentifier};
use uuid::Uuid;

pub const ROUTE_CATEGORY_ID: Uuid = Uuid::from_u128(0x11111111111111111111111111111111);
pub const THEME_ID: Uuid = Uuid::from_u128(0x22222222222222222222222222222222);
pub const ROUTE_COLLECTION_ID: Uuid = Uuid::from_u128(0x33333333333333333333333333333333);
pub const ROUTE_SUMMARY_ID: Uuid = Uuid::from_u128(0x44444444444444444444444444444444);
pub const HIGHLIGHT_ID: Uuid = Uuid::from_u128(0x55555555555555555555555555555555);
pub const COMMUNITY_PICK_ID: Uuid = Uuid::from_u128(0x66666666666666666666666666666666);
pub const EDGE_COMMUNITY_PICK_ID: Uuid = Uuid::from_u128(0x77777777777777777777777777777777);
pub const TAG_ID: Uuid = Uuid::from_u128(0x88888888888888888888888888888888);
pub const BADGE_ID: Uuid = Uuid::from_u128(0x99999999999999999999999999999999);
pub const SAFETY_TOGGLE_ID: Uuid = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
pub const SAFETY_PRESET_ID: Uuid = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
pub const ROUTE_ID: Uuid = Uuid::from_u128(0xcccccccccccccccccccccccccccccccc);
pub const CURATOR_USER_ID: Uuid = Uuid::from_u128(0xdddddddddddddddddddddddddddddddd);

pub fn icon(value: &str) -> SemanticIconIdentifier {
    SemanticIconIdentifier::new(value).expect("icon identifier fixture should be valid")
}

pub fn localizations(name: &str) -> LocalizationMap {
    let mut values = BTreeMap::new();
    values.insert(
        "en-GB".to_owned(),
        LocalizedStringSet::new(name.to_owned(), Some(name.to_owned()), None),
    );
    LocalizationMap::new(values).expect("localization fixture should be valid")
}

pub fn image(url: &str, alt: &str) -> ImageAsset {
    ImageAsset::new(url, alt).expect("image fixture should be valid")
}
