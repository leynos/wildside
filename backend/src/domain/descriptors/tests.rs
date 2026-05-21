//! Unit tests for descriptor domain type construction.

#![cfg(test)]

use std::{collections::BTreeMap, error::Error as StdError};

use rstest::{fixture, rstest};
use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::domain::localization::{LocalizationMap, LocalizedStringSet};
use crate::domain::semantic_icon_identifier::SemanticIconIdentifier;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

fn unwrap_fixtures<T, U>((left, right): (TestResult<T>, TestResult<U>)) -> TestResult<(T, U)> {
    Ok((left?, right?))
}

#[fixture]
fn localizations() -> TestResult<LocalizationMap> {
    let mut values = BTreeMap::new();
    values.insert(
        "en-GB".to_owned(),
        LocalizedStringSet::new("Family friendly", Some("Family".to_owned()), None),
    );
    Ok(LocalizationMap::new(values)?)
}

#[fixture]
fn icon_key() -> TestResult<SemanticIconIdentifier> {
    Ok(SemanticIconIdentifier::new("descriptor:tag")?)
}

#[rstest]
fn tag_accepts_valid_payload(
    localizations: TestResult<LocalizationMap>,
    icon_key: TestResult<SemanticIconIdentifier>,
) -> TestResult {
    let (localizations, icon_key) = unwrap_fixtures((localizations, icon_key))?;
    let tag = Tag::new(Uuid::new_v4(), "family-friendly", icon_key, localizations)?;

    assert_eq!(tag.slug(), "family-friendly");
    Ok(())
}

#[rstest]
fn badge_rejects_invalid_slug(
    localizations: TestResult<LocalizationMap>,
    icon_key: TestResult<SemanticIconIdentifier>,
) -> TestResult {
    let (localizations, icon_key) = unwrap_fixtures((localizations, icon_key))?;
    let result = Badge::new(Uuid::new_v4(), "Family Friendly", icon_key, localizations);

    assert!(matches!(
        result,
        Err(DescriptorValidationError::InvalidSlug {
            field: "badge.slug",
        })
    ));
    Ok(())
}

#[rstest]
fn safety_toggle_accepts_valid_payload(
    localizations: TestResult<LocalizationMap>,
    icon_key: TestResult<SemanticIconIdentifier>,
) -> TestResult {
    let (localizations, icon_key) = unwrap_fixtures((localizations, icon_key))?;
    let toggle = SafetyToggle::new(Uuid::new_v4(), "well-lit", icon_key, localizations)?;

    assert_eq!(toggle.slug(), "well-lit");
    Ok(())
}

fn default_safety_preset_draft(
    icon_key: SemanticIconIdentifier,
    localizations: LocalizationMap,
    safety_toggle_ids: Vec<Uuid>,
) -> SafetyPresetDraft {
    SafetyPresetDraft {
        id: Uuid::new_v4(),
        slug: "quiet-hours".to_owned(),
        icon_key,
        localizations,
        safety_toggle_ids,
    }
}

#[rstest]
fn safety_preset_rejects_empty_toggle_ids(
    localizations: TestResult<LocalizationMap>,
    icon_key: TestResult<SemanticIconIdentifier>,
) -> TestResult {
    let (localizations, icon_key) = unwrap_fixtures((localizations, icon_key))?;
    let result = SafetyPreset::new(default_safety_preset_draft(icon_key, localizations, vec![]));

    assert_eq!(
        result.expect_err("missing toggles should fail"),
        DescriptorValidationError::EmptySafetyPresetToggleIds
    );
    Ok(())
}

#[rstest]
fn safety_preset_rejects_duplicate_toggle_ids(
    localizations: TestResult<LocalizationMap>,
    icon_key: TestResult<SemanticIconIdentifier>,
) -> TestResult {
    let (localizations, icon_key) = unwrap_fixtures((localizations, icon_key))?;
    let toggle_id = Uuid::new_v4();
    let result = SafetyPreset::new(default_safety_preset_draft(
        icon_key,
        localizations,
        vec![toggle_id, toggle_id],
    ));

    assert!(matches!(
        result,
        Err(DescriptorValidationError::DuplicateSafetyPresetToggleId { .. })
    ));
    Ok(())
}

#[rstest]
fn safety_preset_accepts_unique_toggle_ids(
    localizations: TestResult<LocalizationMap>,
    icon_key: TestResult<SemanticIconIdentifier>,
) -> TestResult {
    let (localizations, icon_key) = unwrap_fixtures((localizations, icon_key))?;
    let preset = SafetyPreset::new(default_safety_preset_draft(
        icon_key,
        localizations,
        vec![Uuid::new_v4(), Uuid::new_v4()],
    ))?;

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
