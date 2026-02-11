//! Port abstraction for descriptor ingestion writes.
//!
//! Descriptors are stable registries used by catalogue and user preferences.
//! Keeping these writes behind a domain port preserves adapter boundaries.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{Badge, SafetyPreset, SafetyToggle, Tag};

use super::define_port_error;

define_port_error! {
    /// Errors raised when persisting descriptor ingestion records.
    pub enum DescriptorIngestionRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "descriptor ingestion connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } =>
            "descriptor ingestion query failed: {message}",
    }
}

/// Ingestion payload for interest themes.
#[derive(Debug, Clone)]
pub struct InterestThemeIngestion {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

/// Port for writing descriptor registries.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DescriptorIngestionRepository: Send + Sync {
    async fn upsert_tags(&self, records: &[Tag]) -> Result<(), DescriptorIngestionRepositoryError>;

    async fn upsert_badges(
        &self,
        records: &[Badge],
    ) -> Result<(), DescriptorIngestionRepositoryError>;

    async fn upsert_safety_toggles(
        &self,
        records: &[SafetyToggle],
    ) -> Result<(), DescriptorIngestionRepositoryError>;

    async fn upsert_safety_presets(
        &self,
        records: &[SafetyPreset],
    ) -> Result<(), DescriptorIngestionRepositoryError>;

    async fn upsert_interest_themes(
        &self,
        records: &[InterestThemeIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError>;
}

/// Fixture implementation for tests that do not exercise descriptor writes.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureDescriptorIngestionRepository;

#[async_trait]
impl DescriptorIngestionRepository for FixtureDescriptorIngestionRepository {
    async fn upsert_tags(
        &self,
        _records: &[Tag],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_badges(
        &self,
        _records: &[Badge],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_safety_toggles(
        &self,
        _records: &[SafetyToggle],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_safety_presets(
        &self,
        _records: &[SafetyPreset],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_interest_themes(
        &self,
        _records: &[InterestThemeIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        Ok(())
    }
}
