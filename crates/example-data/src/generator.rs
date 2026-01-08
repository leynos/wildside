//! Deterministic user generation from seed definitions.
//!
//! This module provides the core generation function that produces reproducible
//! user data from a seed registry. The same seed value always produces
//! identical output.

use fake::Fake;
use fake::faker::name::raw::{FirstName, LastName};
use fake::locales::EN;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use uuid::Uuid;

use crate::error::GenerationError;
use crate::registry::{SeedDefinition, SeedRegistry};
use crate::seed::{ExampleUserSeed, UnitSystemSeed};
use crate::validation::{DISPLAY_NAME_MAX, is_valid_display_name, sanitize_display_name};

/// Maximum number of attempts to generate a valid display name.
const MAX_NAME_ATTEMPTS: usize = 100;

/// Minimum number of interest themes to assign to a user.
const MIN_INTEREST_THEMES: usize = 1;

/// Maximum number of interest themes to assign to a user.
const MAX_INTEREST_THEMES: usize = 3;

/// Minimum number of safety toggles to assign to a user.
const MIN_SAFETY_TOGGLES: usize = 0;

/// Maximum number of safety toggles to assign to a user.
const MAX_SAFETY_TOGGLES: usize = 2;

/// Probability of selecting metric units (90%).
const METRIC_PROBABILITY_NUMERATOR: u32 = 9;

/// Probability denominator for unit system selection.
const METRIC_PROBABILITY_DENOMINATOR: u32 = 10;

/// Generates example users from a seed definition.
///
/// Uses the seed's `seed` value to initialise a deterministic RNG, ensuring
/// identical output for the same seed definition. The generated users have:
///
/// - Unique UUIDs (deterministically generated)
/// - Valid display names matching backend constraints
/// - A subset of interest themes from the registry
/// - A subset of safety toggles from the registry
/// - Unit system preference (~90% metric, ~10% imperial)
///
/// # Errors
///
/// Returns [`GenerationError`] if:
/// - Display name generation fails after maximum retries
/// - The registry has no interest themes (required for user generation)
///
/// # Example
///
/// ```
/// use example_data::{SeedRegistry, generate_example_users};
///
/// let json = r#"{
///     "version": 1,
///     "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
///     "safetyToggleIds": [],
///     "seeds": [{"name": "test", "seed": 42, "userCount": 3}]
/// }"#;
///
/// let registry = SeedRegistry::from_json(json).expect("valid");
/// let seed_def = registry.find_seed("test").expect("found");
/// let users = generate_example_users(&registry, seed_def).expect("generated");
///
/// assert_eq!(users.len(), 3);
/// // Same seed produces identical users
/// let users2 = generate_example_users(&registry, seed_def).expect("generated");
/// assert_eq!(users, users2);
/// ```
pub fn generate_example_users(
    registry: &SeedRegistry,
    seed_def: &SeedDefinition,
) -> Result<Vec<ExampleUserSeed>, GenerationError> {
    // Require at least one interest theme
    if registry.interest_theme_ids().is_empty() {
        return Err(GenerationError::NoInterestThemes);
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed_def.seed());
    let mut users = Vec::with_capacity(seed_def.user_count());

    for _ in 0..seed_def.user_count() {
        let user = generate_single_user(&mut rng, registry)?;
        users.push(user);
    }

    Ok(users)
}

/// Generates a single user with the provided RNG.
fn generate_single_user(
    rng: &mut ChaCha8Rng,
    registry: &SeedRegistry,
) -> Result<ExampleUserSeed, GenerationError> {
    // Generate deterministic UUID from RNG
    let id = Uuid::from_u128(rng.random());

    // Generate valid display name
    let display_name = generate_display_name(rng)?;

    // Select interest themes (1-3, or all if fewer available)
    let interest_theme_ids = select_subset(
        rng,
        registry.interest_theme_ids(),
        MIN_INTEREST_THEMES,
        MAX_INTEREST_THEMES,
    );

    // Select safety toggles (0-2, or all if fewer available)
    let safety_toggle_ids = select_subset(
        rng,
        registry.safety_toggle_ids(),
        MIN_SAFETY_TOGGLES,
        MAX_SAFETY_TOGGLES,
    );

    // Select unit system (~90% metric, ~10% imperial)
    let unit_system =
        if rng.random_ratio(METRIC_PROBABILITY_NUMERATOR, METRIC_PROBABILITY_DENOMINATOR) {
            UnitSystemSeed::Metric
        } else {
            UnitSystemSeed::Imperial
        };

    Ok(ExampleUserSeed {
        id,
        display_name,
        interest_theme_ids,
        safety_toggle_ids,
        unit_system,
    })
}

/// Generates a valid display name using the provided RNG.
///
/// Retries up to `MAX_NAME_ATTEMPTS` times if the generated name fails
/// validation. Names are constructed as first name followed by last name,
/// sanitized to remove invalid characters, and truncated if they exceed
/// the maximum length.
fn generate_display_name(rng: &mut ChaCha8Rng) -> Result<String, GenerationError> {
    for _ in 0..MAX_NAME_ATTEMPTS {
        let first: String = FirstName(EN).fake_with_rng(rng);
        let last: String = LastName(EN).fake_with_rng(rng);

        // Combine with space
        let candidate = format!("{first} {last}");

        // Sanitize invalid characters
        let sanitized = sanitize_display_name(&candidate);

        // Truncate if too long (preserving whole characters)
        let truncated: String = sanitized.chars().take(DISPLAY_NAME_MAX).collect();

        if is_valid_display_name(&truncated) {
            return Ok(truncated);
        }
    }

    Err(GenerationError::DisplayNameGenerationFailed {
        max_attempts: MAX_NAME_ATTEMPTS,
    })
}

/// Selects a deterministic subset of IDs from the provided slice.
///
/// The selection count is determined by the RNG state, bounded by `min_count`
/// and `max_count`. If the source slice has fewer elements than `max_count`,
/// all elements may be selected.
fn select_subset(
    rng: &mut ChaCha8Rng,
    ids: &[Uuid],
    min_count: usize,
    max_count: usize,
) -> Vec<Uuid> {
    if ids.is_empty() {
        return Vec::new();
    }

    // Clamp bounds to available IDs
    let clamped_min = min_count.min(ids.len());
    let clamped_max = max_count.min(ids.len());

    // Determine count (handle case where min == max)
    let count = if clamped_min == clamped_max {
        clamped_min
    } else {
        rng.random_range(clamped_min..=clamped_max)
    };

    // Shuffle and take the first `count` elements
    let mut shuffled = ids.to_vec();
    shuffled.shuffle(rng);
    shuffled.truncate(count);
    shuffled
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};

    use super::*;

    /// Generates users from the named seed and asserts a predicate holds for all users.
    ///
    /// # Panics
    ///
    /// Panics if the seed is not found, generation fails, or the predicate
    /// returns `false` for any user.
    fn assert_all_users<F>(registry: &SeedRegistry, seed_name: &str, predicate: F)
    where
        F: Fn(&ExampleUserSeed) -> bool,
    {
        let seed_def = registry.find_seed(seed_name).expect("seed should be found");
        let users = generate_example_users(registry, seed_def).expect("generation should succeed");

        for user in &users {
            assert!(predicate(user), "Predicate failed for user: {user:?}");
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
    fn test_registry() -> SeedRegistry {
        SeedRegistry::from_json(TEST_REGISTRY_JSON).expect("valid test registry")
    }

    #[rstest]
    fn generates_correct_user_count(test_registry: SeedRegistry) {
        let seed_def = test_registry.find_seed("test-seed").expect("seed found");
        let users = generate_example_users(&test_registry, seed_def).expect("generated");

        assert_eq!(users.len(), 10);
    }

    #[rstest]
    fn generation_is_deterministic(test_registry: SeedRegistry) {
        let seed_def = test_registry.find_seed("test-seed").expect("seed found");

        let users1 = generate_example_users(&test_registry, seed_def).expect("generated");
        let users2 = generate_example_users(&test_registry, seed_def).expect("generated");

        assert_eq!(users1, users2);
    }

    #[rstest]
    fn different_seeds_produce_different_users(test_registry: SeedRegistry) {
        let seed1 = test_registry.find_seed("test-seed").expect("seed found");
        let seed2 = test_registry.find_seed("small-seed").expect("seed found");

        let users1 = generate_example_users(&test_registry, seed1).expect("generated");
        let users2 = generate_example_users(&test_registry, seed2).expect("generated");

        // Different seeds should produce different first user IDs
        assert_ne!(users1.first().map(|u| u.id), users2.first().map(|u| u.id));
    }

    #[rstest]
    fn all_display_names_are_valid(test_registry: SeedRegistry) {
        assert_all_users(&test_registry, "test-seed", |user| {
            is_valid_display_name(&user.display_name)
        });
    }

    #[rstest]
    fn interest_themes_are_subset_of_registry(test_registry: SeedRegistry) {
        let registry_ids: std::collections::HashSet<_> =
            test_registry.interest_theme_ids().iter().collect();

        assert_all_users(&test_registry, "test-seed", |user| {
            user.interest_theme_ids
                .iter()
                .all(|id| registry_ids.contains(id))
        });
    }

    #[rstest]
    fn safety_toggles_are_subset_of_registry(test_registry: SeedRegistry) {
        let registry_ids: std::collections::HashSet<_> =
            test_registry.safety_toggle_ids().iter().collect();

        assert_all_users(&test_registry, "test-seed", |user| {
            user.safety_toggle_ids
                .iter()
                .all(|id| registry_ids.contains(id))
        });
    }

    #[rstest]
    fn generates_both_unit_systems(test_registry: SeedRegistry) {
        // Use a seed that produces enough users to likely see both systems
        let seed_def = test_registry.find_seed("test-seed").expect("seed found");
        let users = generate_example_users(&test_registry, seed_def).expect("generated");

        let has_metric = users
            .iter()
            .any(|u| u.unit_system == UnitSystemSeed::Metric);
        let has_imperial = users
            .iter()
            .any(|u| u.unit_system == UnitSystemSeed::Imperial);

        // With 10 users and 90/10 split, we expect both to appear
        assert!(has_metric, "Expected at least one metric user");
        // Note: with 10 users there's a small chance (~35%) of no imperial,
        // but for this specific seed we know the distribution
        assert!(has_imperial, "Expected at least one imperial user");
    }

    #[test]
    fn rejects_registry_without_interest_themes() {
        let json = r#"{
            "version": 1,
            "interestThemeIds": [],
            "safetyToggleIds": [],
            "seeds": [{"name": "test", "seed": 1, "userCount": 1}]
        }"#;
        let registry = SeedRegistry::from_json(json).expect("valid registry");
        let seed_def = registry.find_seed("test").expect("seed found");

        let result = generate_example_users(&registry, seed_def);
        assert_eq!(result, Err(GenerationError::NoInterestThemes));
    }

    #[rstest]
    fn users_have_at_least_one_interest_theme(test_registry: SeedRegistry) {
        assert_all_users(&test_registry, "test-seed", |user| {
            !user.interest_theme_ids.is_empty()
        });
    }

    #[rstest]
    fn users_have_at_most_max_interest_themes(test_registry: SeedRegistry) {
        assert_all_users(&test_registry, "test-seed", |user| {
            user.interest_theme_ids.len() <= MAX_INTEREST_THEMES
        });
    }

    #[rstest]
    fn users_have_at_most_max_safety_toggles(test_registry: SeedRegistry) {
        assert_all_users(&test_registry, "test-seed", |user| {
            user.safety_toggle_ids.len() <= MAX_SAFETY_TOGGLES
        });
    }

    #[test]
    fn select_subset_respects_bounds() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let ids: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();

        for _ in 0..100 {
            let subset = select_subset(&mut rng, &ids, 2, 5);
            assert!(subset.len() >= 2, "Subset too small: {}", subset.len());
            assert!(subset.len() <= 5, "Subset too large: {}", subset.len());
        }
    }

    #[test]
    fn select_subset_handles_empty_slice() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let ids: Vec<Uuid> = vec![];

        let subset = select_subset(&mut rng, &ids, 1, 3);
        assert!(subset.is_empty());
    }

    #[test]
    fn select_subset_clamps_to_available() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let ids: Vec<Uuid> = vec![Uuid::new_v4(), Uuid::new_v4()];

        // Request more than available
        let subset = select_subset(&mut rng, &ids, 5, 10);
        assert!(subset.len() <= 2);
    }
}
