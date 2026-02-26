//! User-state schema coverage audit for roadmap 3.5.1.
//!
//! This module evaluates schema snapshots from the
//! [`SchemaSnapshotRepository`]
//! port and reports whether user-state persistence is covered for:
//! login credentials, users, profile storage, and interests storage.
//!
//! The report also derives migration decisions for:
//! profile storage, interests storage, interests revision tracking, and
//! update conflict handling.

use crate::domain::er_diagram::{SchemaDiagram, SchemaTable};
use crate::domain::ports::{SchemaSnapshotRepository, SchemaSnapshotRepositoryError};

/// Recognised credential columns used by login persistence heuristics.
///
/// Keep this list in sync with schema conventions and migration decisions
/// documented in `docs/user-state-schema-audit-3-5-1.md`.
const LOGIN_CREDENTIAL_COLUMNS: &[&str] = &["password_hash", "password_digest", "credential_hash"];
/// Recognised credential tables used by login persistence heuristics.
const LOGIN_CREDENTIAL_TABLES: &[&str] = &["user_credentials", "login_credentials", "credentials"];

/// Migration requirement derived from audit findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationDecision {
    /// New migration work is still required.
    Required,
    /// Existing schema coverage is sufficient.
    NotRequired,
}

impl MigrationDecision {
    /// Returns true when migration work is required.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::domain::MigrationDecision;
    ///
    /// assert!(MigrationDecision::Required.is_required());
    /// assert!(!MigrationDecision::NotRequired.is_required());
    /// ```
    pub fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }
}

/// Coverage state for entity-backed persistence surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntitySchemaCoverage {
    /// Required tables and columns are present.
    Covered,
    /// Required tables or columns are missing.
    Missing,
}

/// Coverage state for login credential persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginSchemaCoverage {
    /// Credential storage columns/tables are present.
    CredentialsPersisted,
    /// No credential storage is visible in the schema.
    MissingCredentialStorage,
}

/// Coverage shape for user interests persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterestsStorageCoverage {
    /// No interests storage model is present.
    Missing,
    /// Canonical array-backed storage on `user_preferences`.
    CanonicalPreferences,
    /// Canonical join-table storage on `user_interest_themes`.
    CanonicalJoinTable,
    /// Both storage models are present, so migration intent is ambiguous.
    DualModel,
}

/// Result of auditing user-state schema coverage and migration needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStateSchemaAuditReport {
    /// Coverage for persisted login credentials.
    pub login_coverage: LoginSchemaCoverage,
    /// Coverage for core users storage.
    pub users_coverage: EntitySchemaCoverage,
    /// Coverage for profile storage.
    pub profile_coverage: EntitySchemaCoverage,
    /// Coverage shape for interests persistence.
    pub interests_storage_coverage: InterestsStorageCoverage,
    /// Whether interests revision tracking is available.
    pub supports_interests_revision_tracking: bool,
    /// Whether stale-write conflict handling is available for interests.
    pub supports_update_conflict_handling: bool,
    /// Migration decision for profile storage.
    pub profile_storage_migration: MigrationDecision,
    /// Migration decision for interests storage.
    pub interests_storage_migration: MigrationDecision,
    /// Migration decision for interests revision tracking.
    pub interests_revision_tracking_migration: MigrationDecision,
    /// Migration decision for update conflict handling.
    pub update_conflict_handling_migration: MigrationDecision,
}

impl UserStateSchemaAuditReport {
    /// Evaluate a schema snapshot and derive user-state migration decisions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::domain::{MigrationDecision, UserStateSchemaAuditReport};
    /// use backend::domain::er_diagram::{SchemaColumn, SchemaDiagram, SchemaTable};
    ///
    /// let diagram = SchemaDiagram {
    ///     tables: vec![
    ///         SchemaTable {
    ///             name: "users".to_owned(),
    ///             columns: vec![
    ///                 SchemaColumn {
    ///                     name: "id".to_owned(),
    ///                     data_type: "uuid".to_owned(),
    ///                     is_primary_key: true,
    ///                     is_nullable: false,
    ///                 },
    ///                 SchemaColumn {
    ///                     name: "display_name".to_owned(),
    ///                     data_type: "text".to_owned(),
    ///                     is_primary_key: false,
    ///                     is_nullable: false,
    ///                 },
    ///             ],
    ///         },
    ///         SchemaTable {
    ///             name: "user_preferences".to_owned(),
    ///             columns: vec![
    ///                 SchemaColumn {
    ///                     name: "user_id".to_owned(),
    ///                     data_type: "uuid".to_owned(),
    ///                     is_primary_key: true,
    ///                     is_nullable: false,
    ///                 },
    ///                 SchemaColumn {
    ///                     name: "interest_theme_ids".to_owned(),
    ///                     data_type: "uuid[]".to_owned(),
    ///                     is_primary_key: false,
    ///                     is_nullable: false,
    ///                 },
    ///                 SchemaColumn {
    ///                     name: "revision".to_owned(),
    ///                     data_type: "int4".to_owned(),
    ///                     is_primary_key: false,
    ///                     is_nullable: false,
    ///                 },
    ///             ],
    ///         },
    ///     ],
    ///     relationships: vec![],
    /// };
    ///
    /// let report = UserStateSchemaAuditReport::evaluate(&diagram);
    /// assert_eq!(
    ///     report.interests_storage_migration,
    ///     MigrationDecision::NotRequired
    /// );
    /// ```
    pub fn evaluate(diagram: &SchemaDiagram) -> Self {
        let users_coverage = assess_users_coverage(diagram);
        let profile_coverage = users_coverage;
        let login_coverage = assess_login_coverage(diagram);
        let interests_storage_coverage = assess_interests_storage_coverage(diagram);
        let supports_interests_revision_tracking =
            assess_interests_revision_tracking(diagram, interests_storage_coverage);
        let supports_update_conflict_handling = supports_interests_revision_tracking;

        Self {
            login_coverage,
            users_coverage,
            profile_coverage,
            interests_storage_coverage,
            supports_interests_revision_tracking,
            supports_update_conflict_handling,
            profile_storage_migration: decision_for_entity_coverage(profile_coverage),
            interests_storage_migration: decision_for_interests_storage(interests_storage_coverage),
            interests_revision_tracking_migration: decision_for_boolean_capability(
                supports_interests_revision_tracking,
            ),
            update_conflict_handling_migration: decision_for_boolean_capability(
                supports_update_conflict_handling,
            ),
        }
    }
}

/// Domain service for user-state schema coverage audits.
#[derive(Debug, Default, Clone, Copy)]
pub struct UserStateSchemaAuditService;

impl UserStateSchemaAuditService {
    /// Construct a new audit service.
    pub fn new() -> Self {
        Self
    }

    /// Load a schema snapshot and evaluate user-state coverage.
    pub fn audit(
        &self,
        repository: &dyn SchemaSnapshotRepository,
    ) -> Result<UserStateSchemaAuditReport, SchemaSnapshotRepositoryError> {
        audit_user_state_schema_coverage(repository)
    }
}

/// Helper function that audits coverage from a schema snapshot repository.
///
/// # Examples
///
/// ```rust
/// use backend::domain::audit_user_state_schema_coverage;
/// use backend::domain::ports::FixtureSchemaSnapshotRepository;
///
/// let report = audit_user_state_schema_coverage(&FixtureSchemaSnapshotRepository)
///     .expect("fixture schema audit should succeed");
///
/// assert!(report.profile_storage_migration.is_required());
/// ```
pub fn audit_user_state_schema_coverage(
    repository: &dyn SchemaSnapshotRepository,
) -> Result<UserStateSchemaAuditReport, SchemaSnapshotRepositoryError> {
    let diagram = repository.load_schema_diagram()?;
    Ok(UserStateSchemaAuditReport::evaluate(&diagram))
}

fn assess_users_coverage(diagram: &SchemaDiagram) -> EntitySchemaCoverage {
    if table_has_columns(diagram, "users", &["id", "display_name"]) {
        EntitySchemaCoverage::Covered
    } else {
        EntitySchemaCoverage::Missing
    }
}

fn assess_login_coverage(diagram: &SchemaDiagram) -> LoginSchemaCoverage {
    if has_login_credential_storage(diagram) {
        LoginSchemaCoverage::CredentialsPersisted
    } else {
        LoginSchemaCoverage::MissingCredentialStorage
    }
}

fn has_login_credential_storage(diagram: &SchemaDiagram) -> bool {
    if LOGIN_CREDENTIAL_COLUMNS
        .iter()
        .any(|column| table_has_column(diagram, "users", column))
    {
        return true;
    }

    diagram.tables.iter().any(|table| {
        LOGIN_CREDENTIAL_TABLES.contains(&table.name.as_str())
            && table_has_columns_in_table(table, &["user_id"])
            && LOGIN_CREDENTIAL_COLUMNS
                .iter()
                .any(|column| table_has_column_in_table(table, column))
    })
}

fn assess_interests_storage_coverage(diagram: &SchemaDiagram) -> InterestsStorageCoverage {
    let has_preferences_interests = table_has_columns(
        diagram,
        "user_preferences",
        &["user_id", "interest_theme_ids"],
    );
    let has_join_table_interests =
        table_has_columns(diagram, "user_interest_themes", &["user_id", "theme_id"]);

    match (has_preferences_interests, has_join_table_interests) {
        (false, false) => InterestsStorageCoverage::Missing,
        (true, false) => InterestsStorageCoverage::CanonicalPreferences,
        (false, true) => InterestsStorageCoverage::CanonicalJoinTable,
        (true, true) => InterestsStorageCoverage::DualModel,
    }
}

fn assess_interests_revision_tracking(
    diagram: &SchemaDiagram,
    interests_storage_coverage: InterestsStorageCoverage,
) -> bool {
    match interests_storage_coverage {
        InterestsStorageCoverage::CanonicalPreferences => {
            table_has_column(diagram, "user_preferences", "revision")
        }
        InterestsStorageCoverage::CanonicalJoinTable => {
            table_has_column(diagram, "user_interest_themes", "revision")
        }
        InterestsStorageCoverage::Missing | InterestsStorageCoverage::DualModel => false,
    }
}

fn decision_for_entity_coverage(coverage: EntitySchemaCoverage) -> MigrationDecision {
    match coverage {
        EntitySchemaCoverage::Covered => MigrationDecision::NotRequired,
        EntitySchemaCoverage::Missing => MigrationDecision::Required,
    }
}

fn decision_for_interests_storage(coverage: InterestsStorageCoverage) -> MigrationDecision {
    match coverage {
        InterestsStorageCoverage::CanonicalPreferences
        | InterestsStorageCoverage::CanonicalJoinTable => MigrationDecision::NotRequired,
        InterestsStorageCoverage::Missing | InterestsStorageCoverage::DualModel => {
            MigrationDecision::Required
        }
    }
}

fn decision_for_boolean_capability(is_supported: bool) -> MigrationDecision {
    if is_supported {
        MigrationDecision::NotRequired
    } else {
        MigrationDecision::Required
    }
}

fn table_has_columns(diagram: &SchemaDiagram, table_name: &str, columns: &[&str]) -> bool {
    diagram
        .tables
        .iter()
        .find(|table| table.name == table_name)
        .is_some_and(|table| table_has_columns_in_table(table, columns))
}

fn table_has_columns_in_table(table: &SchemaTable, columns: &[&str]) -> bool {
    columns
        .iter()
        .all(|column| table_has_column_in_table(table, column))
}

fn table_has_column(diagram: &SchemaDiagram, table_name: &str, column_name: &str) -> bool {
    diagram
        .tables
        .iter()
        .find(|table| table.name == table_name)
        .is_some_and(|table| table_has_column_in_table(table, column_name))
}

fn table_has_column_in_table(table: &SchemaTable, column_name: &str) -> bool {
    table
        .columns
        .iter()
        .any(|column| column.name == column_name)
}

#[cfg(test)]
#[path = "user_state_schema_audit_tests.rs"]
mod tests;
