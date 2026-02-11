//! Unit tests for descriptor domain type construction.

use std::collections::BTreeMap;

use rstest::rstest;
use uuid::Uuid;

use super::*;
use crate::domain::localization::{LocalizationMap, LocalizedStringSet};
use crate::domain::semantic_icon_identifier::SemanticIconIdentifier;

fn localizations() -> LocalizationMap {
    let mut values = BTreeMap::new();
    values.insert(
        "en-GB".to_owned(),
        LocalizedStringSet::new("Family friendly", Some("Family".to_owned()), None),
    );
    LocalizationMap::new(values).expect("valid localizations")
}

fn icon_key() -> SemanticIconIdentifier {
    SemanticIconIdentifier::new("descriptor:tag").expect("valid icon key")
}

#[rstest]
fn tag_accepts_valid_payload() {
    let tag = Tag::new(
        Uuid::new_v4(),
        "family-friendly",
        icon_key(),
        localizations(),
    )
    .expect("valid tag");

    assert_eq!(tag.slug, "family-friendly");
}

#[rstest]
fn badge_rejects_invalid_slug() {
    let result = Badge::new(
        Uuid::new_v4(),
        "Family Friendly",
        icon_key(),
        localizations(),
    );

    assert!(matches!(
        result,
        Err(DescriptorValidationError::InvalidSlug {
            field: "badge.slug",
        })
    ));
}

#[rstest]
fn safety_toggle_accepts_valid_payload() {
    let toggle = SafetyToggle::new(Uuid::new_v4(), "well-lit", icon_key(), localizations())
        .expect("valid safety toggle");

    assert_eq!(toggle.slug, "well-lit");
}

#[rstest]
fn safety_preset_rejects_empty_toggle_ids() {
    let result = SafetyPreset::new(SafetyPresetDraft {
        id: Uuid::new_v4(),
        slug: "quiet-hours".to_owned(),
        icon_key: icon_key(),
        localizations: localizations(),
        safety_toggle_ids: vec![],
    });

    assert_eq!(
        result.expect_err("missing toggles should fail"),
        DescriptorValidationError::EmptySafetyPresetToggleIds
    );
}

#[rstest]
fn safety_preset_rejects_duplicate_toggle_ids() {
    let toggle_id = Uuid::new_v4();
    let result = SafetyPreset::new(SafetyPresetDraft {
        id: Uuid::new_v4(),
        slug: "quiet-hours".to_owned(),
        icon_key: icon_key(),
        localizations: localizations(),
        safety_toggle_ids: vec![toggle_id, toggle_id],
    });

    assert!(matches!(
        result,
        Err(DescriptorValidationError::DuplicateSafetyPresetToggleId { .. })
    ));
}

#[rstest]
fn safety_preset_accepts_unique_toggle_ids() {
    let preset = SafetyPreset::new(SafetyPresetDraft {
        id: Uuid::new_v4(),
        slug: "quiet-hours".to_owned(),
        icon_key: icon_key(),
        localizations: localizations(),
        safety_toggle_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
    })
    .expect("valid safety preset");

    assert_eq!(preset.safety_toggle_ids.len(), 2);
}
