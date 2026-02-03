//! Port abstraction for applying example data seeds.
//!
//! This port encapsulates the transactional persistence needed to seed example
//! users and their preferences while recording the seed run. Adapters should
//! ensure the seed run insert and user/preference inserts occur atomically.

use async_trait::async_trait;

use crate::domain::{User, UserPreferences};

use super::{SeedingResult, define_port_error};

define_port_error! {
    /// Persistence errors raised by example data seed repository adapters.
    pub enum ExampleDataSeedRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } => "example data seeding connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } => "example data seeding query failed: {message}",
    }
}

/// Seed user record containing both user identity and preferences.
#[derive(Debug, Clone)]
pub struct ExampleDataSeedUser {
    /// Domain user entity.
    pub user: User,
    /// Initial preferences for the user.
    pub preferences: UserPreferences,
}

/// Request payload for applying a seed run.
pub struct ExampleDataSeedRequest {
    /// Seed name recorded in the seed run table.
    pub seed_key: String,
    /// Number of users generated for the seed.
    pub user_count: i32,
    /// RNG seed value used for deterministic generation.
    pub seed: i64,
    /// Generated users and preferences to persist.
    pub users: Vec<ExampleDataSeedUser>,
}

/// Port for applying example data seeds in a single transaction.
///
/// Implementations must:
/// - Insert a seed run record guarded by `ON CONFLICT DO NOTHING`.
/// - Insert or upsert user records.
/// - Insert or upsert user preference records.
/// - Roll back all changes if any step fails.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ExampleDataSeedRepository: Send + Sync {
    /// Apply a seed run and persist the generated example users.
    ///
    /// Returns `Applied` when the seed run is recorded and data is inserted,
    /// or `AlreadySeeded` when the seed key already exists.
    async fn seed_example_data(
        &self,
        request: ExampleDataSeedRequest,
    ) -> Result<SeedingResult, ExampleDataSeedRepositoryError>;
}
