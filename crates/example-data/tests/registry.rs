//! Unit tests for the seed registry module.
//!
//! These tests validate registry parsing, seed lookups, append operations,
//! and file I/O behaviour.

#![expect(
    clippy::expect_used,
    reason = "test code uses expect for clear failure messages"
)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use example_data::{RegistryError, SeedDefinition, SeedRegistry};
use rstest::rstest;

const VALID_JSON: &str = r#"{
    "version": 1,
    "interestThemeIds": [
        "3fa85f64-5717-4562-b3fc-2c963f66afa6",
        "4fa85f64-5717-4562-b3fc-2c963f66afa7"
    ],
    "safetyToggleIds": [
        "7fa85f64-5717-4562-b3fc-2c963f66afa6"
    ],
    "seeds": [
        {"name": "mossy-owl", "seed": 2026, "userCount": 12},
        {"name": "snowy-penguin", "seed": 1234, "userCount": 5}
    ]
}"#;

#[test]
fn parses_valid_registry() {
    let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");

    assert_eq!(registry.version(), 1);
    assert_eq!(registry.interest_theme_ids().len(), 2);
    assert_eq!(registry.safety_toggle_ids().len(), 1);
    assert_eq!(registry.seeds().len(), 2);
}

#[test]
fn finds_seed_by_name() {
    let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");
    let seed = registry.find_seed("mossy-owl").expect("seed found");

    assert_eq!(seed.name(), "mossy-owl");
    assert_eq!(seed.seed(), 2026);
    assert_eq!(seed.user_count(), 12);
}

#[test]
fn returns_error_for_unknown_seed() {
    let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");
    let result = registry.find_seed("unknown");

    assert_eq!(
        result,
        Err(RegistryError::SeedNotFound {
            name: "unknown".to_owned()
        })
    );
}

/// Tests that use pattern matching for parse errors (message content varies).
#[rstest]
#[case::malformed_json("not valid json")]
#[case::missing_version(
    r#"{"interestThemeIds": [], "safetyToggleIds": [], "seeds": [{"name": "a", "seed": 1, "userCount": 1}]}"#
)]
fn rejects_json_with_parse_error(#[case] json: &str) {
    let result = SeedRegistry::from_json(json);
    assert!(matches!(result, Err(RegistryError::ParseError { .. })));
}

/// Tests that check exact error variants.
#[rstest]
#[case::unsupported_version(
    r#"{"version": 99, "interestThemeIds": [], "safetyToggleIds": [], "seeds": [{"name": "a", "seed": 1, "userCount": 1}]}"#,
    RegistryError::UnsupportedVersion { expected: 1, actual: 99 }
)]
#[case::invalid_interest_theme_uuid(
    r#"{"version": 1, "interestThemeIds": ["not-a-uuid"], "safetyToggleIds": [], "seeds": [{"name": "a", "seed": 1, "userCount": 1}]}"#,
    RegistryError::InvalidInterestThemeId { index: 0, value: "not-a-uuid".to_owned() }
)]
#[case::invalid_safety_toggle_uuid(
    r#"{"version": 1, "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"], "safetyToggleIds": ["bad"], "seeds": [{"name": "a", "seed": 1, "userCount": 1}]}"#,
    RegistryError::InvalidSafetyToggleId { index: 0, value: "bad".to_owned() }
)]
#[case::empty_seeds(
    r#"{"version": 1, "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"], "safetyToggleIds": [], "seeds": []}"#,
    RegistryError::EmptySeeds
)]
fn rejects_invalid_registry(#[case] json: &str, #[case] expected: RegistryError) {
    let result = SeedRegistry::from_json(json);
    assert_eq!(result, Err(expected));
}

#[rstest]
#[case("3fa85f64-5717-4562-b3fc-2c963f66afa6")]
#[case("00000000-0000-0000-0000-000000000000")]
fn accepts_valid_uuid_formats(#[case] uuid_str: &str) {
    let json = format!(
        r#"{{"version": 1, "interestThemeIds": ["{uuid_str}"], "safetyToggleIds": [], "seeds": [{{"name": "a", "seed": 1, "userCount": 1}}]}}"#
    );
    let result = SeedRegistry::from_json(&json);
    assert!(result.is_ok());
}

#[test]
fn seed_definition_getters_work() {
    let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");
    let seed = registry.find_seed("snowy-penguin").expect("seed found");

    assert_eq!(seed.name(), "snowy-penguin");
    assert_eq!(seed.seed(), 1234);
    assert_eq!(seed.user_count(), 5);
}

#[test]
fn append_seed_adds_new_seed() {
    let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");
    let new_seed = SeedDefinition::new("autumn-breeze".to_owned(), 77, 4);

    let updated = registry.append_seed(new_seed).expect("append seed");

    assert_eq!(updated.seeds().len(), 3);
    assert!(updated.find_seed("autumn-breeze").is_ok());
}

#[test]
fn append_seed_rejects_duplicate_name() {
    let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");
    let duplicate = SeedDefinition::new("mossy-owl".to_owned(), 77, 4);

    let result = registry.append_seed(duplicate);

    assert_eq!(
        result,
        Err(RegistryError::DuplicateSeedName {
            name: "mossy-owl".to_owned()
        })
    );
}

#[test]
fn serializes_registry_to_pretty_json() {
    let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");

    let json = registry.to_json_pretty().expect("serialize registry");
    let round_trip = SeedRegistry::from_json(&json).expect("round trip");

    assert_eq!(registry, round_trip);
}

#[test]
fn writes_registry_to_file() {
    let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");
    let path = unique_temp_path("seeds.json");

    registry.write_to_file(&path).expect("write registry file");

    let round_trip = SeedRegistry::from_file(&path).expect("load registry");
    assert_eq!(registry, round_trip);

    if let Some(parent) = path.parent() {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "explicitly ignore cleanup failures in test teardown"
        )]
        let _ = fs::remove_dir_all(parent);
    }
}

fn unique_temp_path(file_name: &str) -> PathBuf {
    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir()
        .join("example-data-tests")
        .join(format!("seed-registry-{suffix}-{counter}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir.join(file_name)
}
