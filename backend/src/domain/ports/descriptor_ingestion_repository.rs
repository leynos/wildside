//! Port abstraction for descriptor ingestion writes.
//!
//! Descriptors are stable registries used by catalogue and user preferences.
//! Keeping these writes behind a domain port preserves adapter boundaries.

use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use super::define_port_error;

define_port_error! {
    /// Errors raised when persisting descriptor ingestion payloads.
    pub enum DescriptorIngestionRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "descriptor ingestion connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } =>
            "descriptor ingestion query failed: {message}",
    }
}

/// Ingestion payload for tags.
#[derive(Debug, Clone)]
pub struct TagIngestion {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: Value,
}

/// Ingestion payload for badges.
#[derive(Debug, Clone)]
pub struct BadgeIngestion {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: Value,
}

/// Ingestion payload for safety toggles.
#[derive(Debug, Clone)]
pub struct SafetyToggleIngestion {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: Value,
}

/// Ingestion payload for safety presets.
#[derive(Debug, Clone)]
pub struct SafetyPresetIngestion {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: Value,
    pub safety_toggle_ids: Vec<Uuid>,
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
    async fn upsert_tags(
        &self,
        records: &[TagIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError>;

    async fn upsert_badges(
        &self,
        records: &[BadgeIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError>;

    async fn upsert_safety_toggles(
        &self,
        records: &[SafetyToggleIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError>;

    async fn upsert_safety_presets(
        &self,
        records: &[SafetyPresetIngestion],
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
        _records: &[TagIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_badges(
        &self,
        _records: &[BadgeIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_safety_toggles(
        &self,
        _records: &[SafetyToggleIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_safety_presets(
        &self,
        _records: &[SafetyPresetIngestion],
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
