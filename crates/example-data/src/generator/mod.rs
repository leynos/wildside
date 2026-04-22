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
mod tests;
