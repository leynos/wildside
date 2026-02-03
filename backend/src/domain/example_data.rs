//! Example data seeding orchestration.
//!
//! Converts deterministic example-data registry outputs into domain users and
//! preferences, then delegates persistence to the seeding repository port.

use std::sync::Arc;

use chrono::Utc;
use example_data::{
    ExampleUserSeed, GenerationError, RegistryError, SeedDefinition, SeedRegistry, UnitSystemSeed,
    generate_example_users,
};
use thiserror::Error;

use crate::domain::ports::{
    ExampleDataSeedRepository, ExampleDataSeedRepositoryError, ExampleDataSeedRequest,
    ExampleDataSeedUser, SeedingResult,
};
use crate::domain::{
    DisplayName, UnitSystem, User, UserId, UserPreferencesBuilder, UserValidationError,
};

/// Result of attempting to apply example data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleDataSeedOutcome {
    /// Seed key used to record the run.
    pub seed_key: String,
    /// Number of users generated and persisted.
    pub user_count: usize,
    /// Persistence outcome for the seed run.
    pub result: SeedingResult,
}

/// Errors raised while preparing or applying example data.
#[derive(Debug, Error)]
pub enum ExampleDataSeedingError {
    /// Seed registry lookups failed.
    #[error("seed registry error: {0}")]
    Registry(#[from] RegistryError),
    /// User generation failed.
    #[error("example data generation failed: {0}")]
    Generation(#[from] GenerationError),
    /// Generated display name failed backend validation.
    #[error("generated display name failed validation: {0}")]
    DisplayNameInvalid(#[from] UserValidationError),
    /// Seed value cannot be represented in the database.
    #[error("seed value {seed} exceeds maximum representable value")]
    SeedOverflow { seed: u64 },
    /// User count cannot be represented in the database.
    #[error("user count {count} exceeds maximum representable value")]
    UserCountOverflow { count: usize },
    /// Persistence adapter failed while seeding.
    #[error("example data persistence error: {0}")]
    Persistence(#[from] ExampleDataSeedRepositoryError),
}

/// Service that orchestrates example data seeding.
#[derive(Clone)]
pub struct ExampleDataSeeder<R> {
    repository: Arc<R>,
}

impl<R> ExampleDataSeeder<R> {
    /// Create a new seeder with the given persistence adapter.
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
}

impl<R> ExampleDataSeeder<R>
where
    R: ExampleDataSeedRepository,
{
    /// Apply example data for a named seed within the registry.
    ///
    /// # Errors
    ///
    /// Returns [`ExampleDataSeedingError`] if registry lookup, generation,
    /// validation, or persistence fails.
    pub async fn seed_from_registry(
        &self,
        registry: &SeedRegistry,
        seed_name: &str,
        user_count_override: Option<usize>,
    ) -> Result<ExampleDataSeedOutcome, ExampleDataSeedingError> {
        let seed_def = registry.find_seed(seed_name)?;
        let seed_key = seed_def.name().to_owned();
        let seed_value = seed_def.seed();
        let user_count = user_count_override.unwrap_or(seed_def.user_count());
        let user_count_i32 = i32::try_from(user_count)
            .map_err(|_| ExampleDataSeedingError::UserCountOverflow { count: user_count })?;

        let seed_value_i64 = i64::try_from(seed_value)
            .map_err(|_| ExampleDataSeedingError::SeedOverflow { seed: seed_value })?;

        let seed_def = SeedDefinition::new(seed_key.clone(), seed_value, user_count);
        let example_users = generate_example_users(registry, &seed_def)?;
        let mut users = Vec::with_capacity(example_users.len());
        for seed_user in example_users {
            users.push(convert_seed_user(seed_user)?);
        }

        let request = ExampleDataSeedRequest {
            seed_key: seed_key.clone(),
            user_count: user_count_i32,
            seed: seed_value_i64,
            users,
        };
        let result = self.repository.seed_example_data(request).await?;

        Ok(ExampleDataSeedOutcome {
            seed_key,
            user_count,
            result,
        })
    }
}

fn convert_seed_user(
    seed_user: ExampleUserSeed,
) -> Result<ExampleDataSeedUser, UserValidationError> {
    let user_id = UserId::from_uuid(seed_user.id);
    let display_name = DisplayName::new(seed_user.display_name)?;
    let user = User::new(user_id.clone(), display_name);
    let preferences = UserPreferencesBuilder::new(user_id)
        .interest_theme_ids(seed_user.interest_theme_ids)
        .safety_toggle_ids(seed_user.safety_toggle_ids)
        .unit_system(map_unit_system(seed_user.unit_system))
        .revision(1)
        .updated_at(Utc::now())
        .build();

    Ok(ExampleDataSeedUser { user, preferences })
}

fn map_unit_system(unit_system: UnitSystemSeed) -> UnitSystem {
    match unit_system {
        UnitSystemSeed::Metric => UnitSystem::Metric,
        UnitSystemSeed::Imperial => UnitSystem::Imperial,
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for example data seeding orchestration.

    use super::*;
    use crate::domain::ports::MockExampleDataSeedRepository;
    use rstest::rstest;

    const REGISTRY_JSON: &str = r#"{
        "version": 1,
        "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
        "safetyToggleIds": [],
        "seeds": [{"name": "mossy-owl", "seed": 42, "userCount": 2}]
    }"#;

    fn registry() -> SeedRegistry {
        SeedRegistry::from_json(REGISTRY_JSON).expect("registry should parse")
    }

    #[rstest]
    #[tokio::test]
    async fn seed_applies_for_new_seed() {
        let mut repo = MockExampleDataSeedRepository::new();
        repo.expect_seed_example_data()
            .withf(|request| {
                request.seed_key == "mossy-owl"
                    && request.user_count == 2
                    && request.seed == 42
                    && request.users.len() == 2
            })
            .times(1)
            .return_once(|_| Ok(SeedingResult::Applied));

        let seeder = ExampleDataSeeder::new(Arc::new(repo));
        let outcome = seeder
            .seed_from_registry(&registry(), "mossy-owl", None)
            .await
            .expect("seed succeeds");

        assert_eq!(outcome.result, SeedingResult::Applied);
        assert_eq!(outcome.user_count, 2);
        assert_eq!(outcome.seed_key, "mossy-owl");
    }

    #[rstest]
    #[tokio::test]
    async fn seed_skips_when_already_seeded() {
        let mut repo = MockExampleDataSeedRepository::new();
        repo.expect_seed_example_data()
            .times(1)
            .return_once(|_| Ok(SeedingResult::AlreadySeeded));

        let seeder = ExampleDataSeeder::new(Arc::new(repo));
        let outcome = seeder
            .seed_from_registry(&registry(), "mossy-owl", None)
            .await
            .expect("seed succeeds");

        assert_eq!(outcome.result, SeedingResult::AlreadySeeded);
    }

    #[rstest]
    #[tokio::test]
    async fn seed_rejects_unknown_seed() {
        let seeder = ExampleDataSeeder::new(Arc::new(MockExampleDataSeedRepository::new()));
        let error = seeder
            .seed_from_registry(&registry(), "missing-seed", None)
            .await
            .expect_err("missing seed should error");

        assert!(matches!(error, ExampleDataSeedingError::Registry(_)));
    }

    #[rstest]
    #[tokio::test]
    async fn user_count_overflow_is_rejected() {
        let mut repo = MockExampleDataSeedRepository::new();
        repo.expect_seed_example_data().times(0);

        let seeder = ExampleDataSeeder::new(Arc::new(repo));
        let overflow_count = (i32::MAX as usize) + 1;
        let error = seeder
            .seed_from_registry(&registry(), "mossy-owl", Some(overflow_count))
            .await
            .expect_err("overflow should be rejected");

        assert!(matches!(
            error,
            ExampleDataSeedingError::UserCountOverflow { count } if count == overflow_count
        ));
    }

    #[rstest]
    fn convert_seed_user_rejects_invalid_display_name() {
        let seed_user = ExampleUserSeed {
            id: uuid::Uuid::new_v4(),
            display_name: "!!".to_owned(),
            interest_theme_ids: Vec::new(),
            safety_toggle_ids: Vec::new(),
            unit_system: UnitSystemSeed::Metric,
        };

        let result = convert_seed_user(seed_user);
        assert!(matches!(
            result,
            Err(UserValidationError::DisplayNameTooShort { .. })
                | Err(UserValidationError::DisplayNameInvalidCharacters)
        ));
    }
}
