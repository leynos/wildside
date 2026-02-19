//! Read-side port for descriptor registry retrieval.
//!
//! Descriptors are shared reference data (tags, badges, safety toggles,
//! safety presets, interest themes) consumed by the PWA for settings,
//! filtering, and display.  This port keeps retrieval behind the hexagonal
//! boundary so inbound adapters depend only on domain types.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::{Badge, Error, InterestTheme, SafetyPreset, SafetyToggle, Tag};

use super::define_port_error;

define_port_error! {
    /// Errors raised when reading descriptor snapshots.
    pub enum DescriptorRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "descriptor read connection failed: {message}",
        /// Query failed during execution or row conversion.
        Query { message: String } =>
            "descriptor read query failed: {message}",
    }
}

/// Cohesive snapshot of all descriptor registries.
#[derive(Debug, Clone)]
pub struct DescriptorSnapshot {
    pub generated_at: DateTime<Utc>,
    pub tags: Vec<Tag>,
    pub badges: Vec<Badge>,
    pub safety_toggles: Vec<SafetyToggle>,
    pub safety_presets: Vec<SafetyPreset>,
    pub interest_themes: Vec<InterestTheme>,
}

impl DescriptorSnapshot {
    /// Construct an empty snapshot for fixture and fallback paths.
    pub fn empty() -> Self {
        Self {
            generated_at: DateTime::<Utc>::default(),
            tags: Vec::new(),
            badges: Vec::new(),
            safety_toggles: Vec::new(),
            safety_presets: Vec::new(),
            interest_themes: Vec::new(),
        }
    }
}

/// Port for reading descriptor registries.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DescriptorRepository: Send + Sync {
    /// Return the current descriptor snapshot.
    ///
    /// All collections are deterministically ordered (by slug or name).
    /// Empty tables yield empty vectors rather than errors.
    ///
    /// # Examples
    ///
    /// ```
    /// # use backend::domain::ports::{DescriptorRepository, FixtureDescriptorRepository};
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let repo = FixtureDescriptorRepository;
    /// let snapshot = repo.descriptor_snapshot().await.unwrap();
    /// assert!(snapshot.tags.is_empty());
    /// assert!(snapshot.safety_presets.is_empty());
    /// # });
    /// ```
    async fn descriptor_snapshot(&self) -> Result<DescriptorSnapshot, DescriptorRepositoryError>;
}

/// Fixture implementation for tests that do not exercise descriptor reads.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureDescriptorRepository;

#[async_trait]
impl DescriptorRepository for FixtureDescriptorRepository {
    async fn descriptor_snapshot(&self) -> Result<DescriptorSnapshot, DescriptorRepositoryError> {
        Ok(DescriptorSnapshot::empty())
    }
}

impl From<DescriptorRepositoryError> for Error {
    fn from(err: DescriptorRepositoryError) -> Self {
        match err {
            DescriptorRepositoryError::Connection { message } => {
                Error::service_unavailable(message)
            }
            DescriptorRepositoryError::Query { message } => Error::internal(message),
        }
    }
}
