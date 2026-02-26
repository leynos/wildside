//! Unit tests for user-state schema audit coverage decisions.

use rstest::rstest;

use super::{
    EntitySchemaCoverage, InterestsStorageCoverage, LoginSchemaCoverage, MigrationDecision,
    UserStateSchemaAuditReport, UserStateSchemaAuditService, audit_user_state_schema_coverage,
};
use crate::domain::er_diagram::{SchemaColumn, SchemaDiagram, SchemaTable};
use crate::domain::ports::{MockSchemaSnapshotRepository, SchemaSnapshotRepositoryError};

#[rstest]
fn baseline_schema_reports_login_gap_and_interests_migrations() {
    let report = UserStateSchemaAuditReport::evaluate(&baseline_diagram());

    assert_eq!(
        report.login_coverage,
        LoginSchemaCoverage::MissingCredentialStorage
    );
    assert_eq!(report.users_coverage, EntitySchemaCoverage::Covered);
    assert_eq!(report.profile_coverage, EntitySchemaCoverage::Covered);
    assert_eq!(
        report.interests_storage_coverage,
        InterestsStorageCoverage::DualModel
    );
    assert!(!report.supports_interests_revision_tracking);
    assert!(!report.supports_update_conflict_handling);
    assert_eq!(
        report.profile_storage_migration,
        MigrationDecision::NotRequired
    );
    assert_eq!(
        report.interests_storage_migration,
        MigrationDecision::Required
    );
    assert_eq!(
        report.interests_revision_tracking_migration,
        MigrationDecision::Required
    );
    assert_eq!(
        report.update_conflict_handling_migration,
        MigrationDecision::Required
    );
}

#[rstest]
fn missing_users_table_requires_users_and_profile_migrations() {
    let report = UserStateSchemaAuditReport::evaluate(&diagram(vec![table(
        "user_preferences",
        &["user_id", "interest_theme_ids", "revision"],
    )]));

    assert_eq!(report.users_coverage, EntitySchemaCoverage::Missing);
    assert_eq!(report.profile_coverage, EntitySchemaCoverage::Missing);
    assert_eq!(
        report.profile_storage_migration,
        MigrationDecision::Required
    );
}

#[rstest]
fn canonical_revisioned_interests_model_needs_no_interests_migrations() {
    let report = UserStateSchemaAuditReport::evaluate(&canonical_interests_diagram());

    assert_eq!(
        report.interests_storage_coverage,
        InterestsStorageCoverage::CanonicalPreferences
    );
    assert!(report.supports_interests_revision_tracking);
    assert!(report.supports_update_conflict_handling);
    assert_eq!(
        report.interests_storage_migration,
        MigrationDecision::NotRequired
    );
    assert_eq!(
        report.interests_revision_tracking_migration,
        MigrationDecision::NotRequired
    );
    assert_eq!(
        report.update_conflict_handling_migration,
        MigrationDecision::NotRequired
    );
}

#[rstest]
fn audit_service_loads_diagram_from_repository() {
    let mut repository = MockSchemaSnapshotRepository::new();
    repository
        .expect_load_schema_diagram()
        .times(1)
        .return_once(|| Ok(canonical_interests_diagram()));

    let report = UserStateSchemaAuditService::new()
        .audit(&repository)
        .expect("audit should succeed");

    assert_eq!(
        report.interests_storage_migration,
        MigrationDecision::NotRequired
    );
}

#[rstest]
fn audit_helper_propagates_repository_errors() {
    let mut repository = MockSchemaSnapshotRepository::new();
    repository
        .expect_load_schema_diagram()
        .times(1)
        .return_once(|| Err(SchemaSnapshotRepositoryError::query("catalog failure")));

    let result = audit_user_state_schema_coverage(&repository);
    assert!(matches!(
        result,
        Err(SchemaSnapshotRepositoryError::Query { .. })
    ));
}

fn baseline_diagram() -> SchemaDiagram {
    diagram(vec![
        table("users", &["id", "display_name"]),
        table(
            "user_preferences",
            &["user_id", "interest_theme_ids", "revision"],
        ),
        table("user_interest_themes", &["user_id", "theme_id"]),
    ])
}

fn canonical_interests_diagram() -> SchemaDiagram {
    diagram(vec![
        table("users", &["id", "display_name"]),
        table(
            "user_preferences",
            &["user_id", "interest_theme_ids", "revision"],
        ),
    ])
}

fn diagram(tables: Vec<SchemaTable>) -> SchemaDiagram {
    SchemaDiagram {
        tables,
        relationships: vec![],
    }
}

fn table(name: &str, columns: &[&str]) -> SchemaTable {
    SchemaTable {
        name: name.to_owned(),
        columns: columns
            .iter()
            .map(|column_name| SchemaColumn {
                name: (*column_name).to_owned(),
                data_type: "text".to_owned(),
                is_primary_key: *column_name == "id" || *column_name == "user_id",
                is_nullable: false,
            })
            .collect(),
    }
}
