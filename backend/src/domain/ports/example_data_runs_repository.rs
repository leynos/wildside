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
        /// Seed value exceeds the representable range (i64::MAX).
        SeedOverflow { seed: u64 } => "seed value {seed} exceeds maximum representable value",
    }
}

/// Convert a u64 seed value to i64, returning an error if it overflows.
///
/// The database stores seed values as BIGINT (i64). This function performs
/// a checked conversion from u64 (as used by `SeedDefinition::seed()`) to
/// i64, rejecting values above `i64::MAX` with a clear domain error.
///
/// # Errors
///
/// Returns `ExampleDataRunsError::SeedOverflow` if `seed > i64::MAX`.
///
/// # Examples
///
/// ```
/// use backend::domain::ports::{try_seed_to_i64, ExampleDataRunsError};
///
/// // Successful conversion
/// assert_eq!(try_seed_to_i64(42).unwrap(), 42);
///
/// // Overflow error for values exceeding i64::MAX
/// let result = try_seed_to_i64(u64::MAX);
/// assert!(matches!(result, Err(ExampleDataRunsError::SeedOverflow { .. })));
/// ```
pub fn try_seed_to_i64(seed: u64) -> Result<i64, ExampleDataRunsError> {
    i64::try_from(seed).map_err(|_| ExampleDataRunsError::seed_overflow(seed))
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
    //! Regression coverage for this module.
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

    #[rstest]
    fn try_seed_to_i64_converts_valid_values() {
        assert_eq!(try_seed_to_i64(0).expect("convert 0 to i64"), 0);
        assert_eq!(try_seed_to_i64(42).expect("convert 42 to i64"), 42);
        assert_eq!(
            try_seed_to_i64(i64::MAX as u64).expect("convert i64::MAX to i64"),
            i64::MAX
        );
    }

    #[rstest]
    fn try_seed_to_i64_rejects_overflow() {
        let overflow_seed = (i64::MAX as u64) + 1;
        let result = try_seed_to_i64(overflow_seed);
        assert!(matches!(
            result,
            Err(ExampleDataRunsError::SeedOverflow { seed }) if seed == overflow_seed
        ));
    }

    #[rstest]
    fn try_seed_to_i64_rejects_max_u64() {
        let result = try_seed_to_i64(u64::MAX);
        assert!(matches!(
            result,
            Err(ExampleDataRunsError::SeedOverflow { seed }) if seed == u64::MAX
        ));
    }
}
