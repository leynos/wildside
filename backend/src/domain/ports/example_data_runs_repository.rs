//! Port abstraction for tracking example data seed runs.
//!
//! This port provides the interface for recording and querying which example
//! data seeds have been applied to the database. It guards against duplicate
//! seeding on concurrent startups or restarts.

use async_trait::async_trait;

use super::define_port_error;

define_port_error! {
    /// Persistence errors raised by example data runs repository adapters.
    pub enum ExampleDataRunsError {
        /// Repository connection could not be established.
        Connection { message: String } => "example data runs connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } => "example data runs query failed: {message}",
    }
}

/// Result of attempting to record a seed run.
///
/// This enum distinguishes between a newly applied seed and one that was
/// already recorded, allowing callers to skip seeding without treating
/// the latter as an error condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedingResult {
    /// Seed was newly recorded; proceed with seeding.
    Applied,
    /// Seed was already recorded; skip seeding.
    AlreadySeeded,
}

/// Port for tracking example data seed runs.
///
/// Implementations must provide idempotent seed recording and lookup
/// functionality. The `try_record_seed` method should use database-level
/// conflict handling to ensure exactly-once semantics even under concurrent
/// access.
///
/// # Example
///
/// ```ignore
/// let result = repo.try_record_seed("mossy-owl", 12, 2026).await?;
/// match result {
///     SeedingResult::Applied => {
///         // Proceed with user/preference insertion
///     }
///     SeedingResult::AlreadySeeded => {
///         // Log and skip
///     }
/// }
/// ```
#[async_trait]
pub trait ExampleDataRunsRepository: Send + Sync {
    /// Attempt to record a seed run.
    ///
    /// Returns `Applied` if the record was inserted (seed not previously run),
    /// or `AlreadySeeded` if it already exists (seed was previously run).
    ///
    /// # Arguments
    ///
    /// * `seed_key` - The seed name (e.g., "mossy-owl")
    /// * `user_count` - Number of users created by this seed
    /// * `seed` - The RNG seed value used for deterministic generation
    async fn try_record_seed(
        &self,
        seed_key: &str,
        user_count: i32,
        seed: i64,
    ) -> Result<SeedingResult, ExampleDataRunsError>;

    /// Check if a seed has already been applied.
    ///
    /// Returns `true` if the seed exists in the database, `false` otherwise.
    async fn is_seeded(&self, seed_key: &str) -> Result<bool, ExampleDataRunsError>;
}

/// Test fixture implementation that always reports seeds as not yet applied.
///
/// Useful for unit testing code that depends on the repository without
/// requiring a real database connection.
#[derive(Debug, Default, Clone)]
pub struct FixtureExampleDataRunsRepository;

#[async_trait]
impl ExampleDataRunsRepository for FixtureExampleDataRunsRepository {
    async fn try_record_seed(
        &self,
        _seed_key: &str,
        _user_count: i32,
        _seed: i64,
    ) -> Result<SeedingResult, ExampleDataRunsError> {
        Ok(SeedingResult::Applied)
    }

    async fn is_seeded(&self, _seed_key: &str) -> Result<bool, ExampleDataRunsError> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[tokio::test]
    async fn fixture_repository_returns_applied() {
        let repo = FixtureExampleDataRunsRepository;
        let result = repo.try_record_seed("test-seed", 10, 42).await;
        assert!(matches!(result, Ok(SeedingResult::Applied)));
    }

    #[rstest]
    #[tokio::test]
    async fn fixture_repository_returns_not_seeded() {
        let repo = FixtureExampleDataRunsRepository;
        let result = repo.is_seeded("test-seed").await;
        assert!(matches!(result, Ok(false)));
    }
}
