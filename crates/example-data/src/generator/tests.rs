//! Regression tests for deterministic example user generation.

use rstest::{fixture, rstest};

use super::*;
use crate::error::RegistryError;

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct SelectSubsetCase {
    ids: Vec<Uuid>,
    min_count: usize,
    max_count: usize,
    iterations: usize,
    expected_min: usize,
    expected_max: usize,
    is_full_source_expected: bool,
}
type RegistryResult = Result<SeedRegistry, RegistryError>;

/// Generates users from the named seed and checks that `predicate` holds for each.
///
/// Returns an error if the seed is not found, generation fails, or the
/// predicate returns `false` for any user.
fn check_all_users<F>(registry: &SeedRegistry, seed_name: &str, predicate: F) -> TestResult
where
    F: Fn(&ExampleUserSeed) -> bool,
{
    let seed_def = registry.find_seed(seed_name)?;
    let users = generate_example_users(registry, seed_def)?;
    for user in &users {
        if !predicate(user) {
            return Err(format!("Predicate failed for user: {user:?}").into());
        }
    }
    Ok(())
}

fn check_eq<T: PartialEq + std::fmt::Debug>(actual: &T, expected: &T, context: &str) -> TestResult {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{context}: expected {expected:?}, got {actual:?}").into())
    }
}

fn check_ne<T: PartialEq + std::fmt::Debug>(lhs: &T, rhs: &T, context: &str) -> TestResult {
    if lhs == rhs {
        Err(format!("{context}: values unexpectedly equal ({lhs:?})").into())
    } else {
        Ok(())
    }
}

const TEST_REGISTRY_JSON: &str = r#"{
    "version": 1,
    "interestThemeIds": [
        "3fa85f64-5717-4562-b3fc-2c963f66afa6",
        "4fa85f64-5717-4562-b3fc-2c963f66afa7",
        "5fa85f64-5717-4562-b3fc-2c963f66afa8"
    ],
    "safetyToggleIds": [
        "7fa85f64-5717-4562-b3fc-2c963f66afa6",
        "8fa85f64-5717-4562-b3fc-2c963f66afa7"
    ],
    "seeds": [
        {"name": "test-seed", "seed": 42, "userCount": 10},
        {"name": "small-seed", "seed": 123, "userCount": 2}
    ]
}"#;

#[fixture]
fn test_registry() -> RegistryResult {
    SeedRegistry::from_json(TEST_REGISTRY_JSON)
}

#[rstest]
fn generates_correct_user_count(test_registry: RegistryResult) -> TestResult {
    let registry = test_registry?;
    let seed_def = registry.find_seed("test-seed")?;
    let users = generate_example_users(&registry, seed_def)?;
    check_eq(&users.len(), &10, "user count")
}

#[rstest]
fn generation_is_deterministic(test_registry: RegistryResult) -> TestResult {
    let registry = test_registry?;
    let seed_def = registry.find_seed("test-seed")?;
    let users1 = generate_example_users(&registry, seed_def)?;
    let users2 = generate_example_users(&registry, seed_def)?;
    check_eq(&users1, &users2, "repeated generation")
}

#[rstest]
fn different_seeds_produce_different_users(test_registry: RegistryResult) -> TestResult {
    let registry = test_registry?;
    let seed1 = registry.find_seed("test-seed")?;
    let seed2 = registry.find_seed("small-seed")?;
    let users1 = generate_example_users(&registry, seed1)?;
    let users2 = generate_example_users(&registry, seed2)?;
    // Different seeds should produce different first user IDs
    check_ne(
        &users1.first().map(|u| u.id),
        &users2.first().map(|u| u.id),
        "first user id across seeds",
    )
}

#[rstest]
fn all_display_names_are_valid(test_registry: RegistryResult) -> TestResult {
    let registry = test_registry?;
    check_all_users(&registry, "test-seed", |user| {
        is_valid_display_name(&user.display_name)
    })
}

#[rstest]
fn interest_themes_are_subset_of_registry(test_registry: RegistryResult) -> TestResult {
    let registry = test_registry?;
    let registry_ids: std::collections::HashSet<_> = registry.interest_theme_ids().iter().collect();
    check_all_users(&registry, "test-seed", |user| {
        user.interest_theme_ids
            .iter()
            .all(|id| registry_ids.contains(id))
    })
}

#[rstest]
fn safety_toggles_are_subset_of_registry(test_registry: RegistryResult) -> TestResult {
    let registry = test_registry?;
    let registry_ids: std::collections::HashSet<_> = registry.safety_toggle_ids().iter().collect();
    check_all_users(&registry, "test-seed", |user| {
        user.safety_toggle_ids
            .iter()
            .all(|id| registry_ids.contains(id))
    })
}

#[test]
fn select_unit_system_can_produce_both_variants() {
    // Drive `select_unit_system` directly so the assertion does not depend on
    // the full RNG trace used by `generate_example_users` (name generation,
    // subset selection, and so on).
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let mut has_seen_metric = false;
    let mut has_seen_imperial = false;
    // With a 9:1 metric:imperial ratio, 200 draws reliably cover both variants
    // for a fixed seed while keeping the test deterministic.
    for _ in 0..200 {
        match select_unit_system(&mut rng) {
            UnitSystemSeed::Metric => has_seen_metric = true,
            UnitSystemSeed::Imperial => has_seen_imperial = true,
        }
        if has_seen_metric && has_seen_imperial {
            return;
        }
    }
    panic!(
        "select_unit_system failed to produce both variants: \
         metric={has_seen_metric}, imperial={has_seen_imperial}"
    );
}

#[test]
fn rejects_registry_without_interest_themes_at_parse_time() {
    let json = r#"{
        "version": 1,
        "interestThemeIds": [],
        "safetyToggleIds": [],
        "seeds": [{"name": "test", "seed": 1, "userCount": 1}]
    }"#;
    let result = SeedRegistry::from_json(json);
    assert_eq!(result, Err(RegistryError::EmptyInterestThemes));
}

#[rstest]
fn users_have_at_least_one_interest_theme(test_registry: RegistryResult) -> TestResult {
    let registry = test_registry?;
    check_all_users(&registry, "test-seed", |user| {
        !user.interest_theme_ids.is_empty()
    })
}

#[rstest]
fn users_have_at_most_max_interest_themes(test_registry: RegistryResult) -> TestResult {
    let registry = test_registry?;
    check_all_users(&registry, "test-seed", |user| {
        user.interest_theme_ids.len() <= MAX_INTEREST_THEMES
    })
}

#[rstest]
fn users_have_at_most_max_safety_toggles(test_registry: RegistryResult) -> TestResult {
    let registry = test_registry?;
    check_all_users(&registry, "test-seed", |user| {
        user.safety_toggle_ids.len() <= MAX_SAFETY_TOGGLES
    })
}

#[rstest]
#[case::respects_bounds(SelectSubsetCase {
    ids: deterministic_ids(10),
    min_count: 2,
    max_count: 5,
    iterations: 100,
    expected_min: 2,
    expected_max: 5,
    is_full_source_expected: false,
})]
#[case::normalizes_inverted_bounds(SelectSubsetCase {
    ids: deterministic_ids(10),
    min_count: 5,
    max_count: 2,
    iterations: 100,
    expected_min: 2,
    expected_max: 5,
    is_full_source_expected: false,
})]
#[case::handles_empty_slice(SelectSubsetCase {
    ids: Vec::new(),
    min_count: 1,
    max_count: 3,
    iterations: 1,
    expected_min: 0,
    expected_max: 0,
    is_full_source_expected: false,
})]
#[case::clamps_to_available(SelectSubsetCase {
    ids: deterministic_ids(2),
    min_count: 5,
    max_count: 10,
    iterations: 1,
    expected_min: 2,
    expected_max: 2,
    is_full_source_expected: true,
})]
fn select_subset_handles_expected_bounds(#[case] case: SelectSubsetCase) {
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let source_ids = case
        .ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();

    for _ in 0..case.iterations {
        let subset = select_subset(&mut rng, &case.ids, case.min_count, case.max_count);

        assert!(
            subset.len() >= case.expected_min,
            "Subset too small: {}",
            subset.len()
        );
        assert!(
            subset.len() <= case.expected_max,
            "Subset too large: {}",
            subset.len()
        );

        assert!(
            subset.iter().all(|id| source_ids.contains(id)),
            "Subset contains IDs outside the source collection"
        );

        if case.is_full_source_expected {
            assert_eq!(subset.len(), case.ids.len());
            assert_eq!(
                subset
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>(),
                source_ids
            );
        }
    }
}

fn deterministic_ids(count: u128) -> Vec<Uuid> {
    (0..count).map(Uuid::from_u128).collect()
}
