//! Unit tests for descriptor domain type construction.

use std::{collections::BTreeMap, error::Error as StdError};

use rstest::rstest;
use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::domain::localization::{LocalizationMap, LocalizedStringSet};
use crate::domain::semantic_icon_identifier::SemanticIconIdentifier;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

fn localizations() -> TestResult<LocalizationMap> {
    let mut values = BTreeMap::new();
    values.insert(
        "en-GB".to_owned(),
        LocalizedStringSet::new("Family friendly", Some("Family".to_owned()), None),
    );
    Ok(LocalizationMap::new(values)?)
}

fn icon_key() -> TestResult<SemanticIconIdentifier> {
    Ok(SemanticIconIdentifier::new("descriptor:tag")?)
}

#[rstest]
fn tag_accepts_valid_payload() -> TestResult {
    let tag = Tag::new(
        Uuid::new_v4(),
        "family-friendly",
        icon_key()?,
        localizations()?,
    )?;

    assert_eq!(tag.slug(), "family-friendly");
    Ok(())
}

#[rstest]
fn badge_rejects_invalid_slug() -> TestResult {
    let result = Badge::new(
        Uuid::new_v4(),
        "Family Friendly",
        icon_key()?,
        localizations()?,
    );

    assert!(matches!(
        result,
        Err(DescriptorValidationError::InvalidSlug {
            field: "badge.slug",
        })
    ));
    Ok(())
}

#[rstest]
fn safety_toggle_accepts_valid_payload() -> TestResult {
    let toggle = SafetyToggle::new(Uuid::new_v4(), "well-lit", icon_key()?, localizations()?)?;

    assert_eq!(toggle.slug(), "well-lit");
    Ok(())
}

#[rstest]
fn safety_preset_rejects_empty_toggle_ids() -> TestResult {
    let result = SafetyPreset::new(SafetyPresetDraft {
        id: Uuid::new_v4(),
        slug: "quiet-hours".to_owned(),
        icon_key: icon_key()?,
        localizations: localizations()?,
        safety_toggle_ids: vec![],
    });

    assert_eq!(
        result.expect_err("missing toggles should fail"),
        DescriptorValidationError::EmptySafetyPresetToggleIds
    );
    Ok(())
}

#[rstest]
fn safety_preset_rejects_duplicate_toggle_ids() -> TestResult {
    let toggle_id = Uuid::new_v4();
    let result = SafetyPreset::new(SafetyPresetDraft {
        id: Uuid::new_v4(),
        slug: "quiet-hours".to_owned(),
        icon_key: icon_key()?,
        localizations: localizations()?,
        safety_toggle_ids: vec![toggle_id, toggle_id],
    });

    assert!(matches!(
        result,
        Err(DescriptorValidationError::DuplicateSafetyPresetToggleId { .. })
    ));
    Ok(())
}

#[rstest]
fn safety_preset_accepts_unique_toggle_ids() -> TestResult {
    let preset = SafetyPreset::new(SafetyPresetDraft {
        id: Uuid::new_v4(),
        slug: "quiet-hours".to_owned(),
        icon_key: icon_key()?,
        localizations: localizations()?,
        safety_toggle_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
    })?;

    assert_eq!(preset.safety_toggle_ids().len(), 2);
    Ok(())
}

#[rstest]
fn safety_preset_deserialization_uses_validating_constructor() {
    let payload = json!({
        "id": Uuid::new_v4(),
        "slug": "invalid slug",
        "iconKey": "descriptor:tag",
        "localizations": {
            "en-GB": {
                "name": "Night safe"
            }
        },
        "safetyToggleIds": [Uuid::new_v4()]
    });

    let err = serde_json::from_value::<SafetyPreset>(payload)
        .expect_err("invalid slug should fail deserialization");
    assert!(err.to_string().contains("lowercase ASCII letters"));
}
