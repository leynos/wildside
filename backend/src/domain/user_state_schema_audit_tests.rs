//! Unit tests for user-state schema audit coverage decisions.

use rstest::rstest;

use super::{
    EntitySchemaCoverage, InterestsStorageCoverage, LoginSchemaCoverage, MigrationDecision,
    UserStateSchemaAuditReport, UserStateSchemaAuditService, audit_user_state_schema_coverage,
};
use crate::domain::er_diagram::{SchemaColumn, SchemaDiagram, SchemaTable};
use crate::domain::ports::{MockSchemaSnapshotRepository, SchemaSnapshotRepositoryError};

// Custom assertion helpers for common test criteria patterns

/// Assert that revision tracking and conflict handling are both supported.
fn assert_supports_revision_and_conflict_handling(report: &UserStateSchemaAuditReport) {
    assert!(report.supports_interests_revision_tracking);
    assert!(report.supports_update_conflict_handling);
}

/// Assert that all interests-related migrations are not required.
fn assert_no_interests_migrations_required(report: &UserStateSchemaAuditReport) {
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

fn assert_all_interests_migrations_required(report: &UserStateSchemaAuditReport) {
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

fn assert_interests_coverage_with_revision_support(
    report: &UserStateSchemaAuditReport,
    coverage: InterestsStorageCoverage,
    supports_revision: bool,
) {
    assert_eq!(report.interests_storage_coverage, coverage);
    assert_eq!(
        report.supports_interests_revision_tracking,
        supports_revision
    );
    assert_eq!(report.supports_update_conflict_handling, supports_revision);
}

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
    assert_supports_revision_and_conflict_handling(&report);
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
        MigrationDecision::NotRequired
    );
    assert_eq!(
        report.update_conflict_handling_migration,
        MigrationDecision::NotRequired
    );
}

#[rstest]
#[case(users_with_inline_credentials_diagram)]
#[case(separate_credentials_table_diagram)]
fn credential_storage_variants_are_recognised_as_persisted(
    #[case] diagram_provider: fn() -> SchemaDiagram,
) {
    let report = UserStateSchemaAuditReport::evaluate(&diagram_provider());

    assert_eq!(
        report.login_coverage,
        LoginSchemaCoverage::CredentialsPersisted
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
    assert_supports_revision_and_conflict_handling(&report);
    assert_no_interests_migrations_required(&report);
}

#[rstest]
fn missing_interests_tables_require_interests_migrations() {
    let report = UserStateSchemaAuditReport::evaluate(&no_interests_diagram());

    assert_interests_coverage_with_revision_support(
        &report,
        InterestsStorageCoverage::Missing,
        false,
    );
    assert_all_interests_migrations_required(&report);
}

#[rstest]
fn canonical_join_interests_without_revision_require_revision_migrations() {
    let report = UserStateSchemaAuditReport::evaluate(&canonical_join_interests_diagram(false));

    assert_interests_coverage_with_revision_support(
        &report,
        InterestsStorageCoverage::CanonicalJoinTable,
        false,
    );
    assert_eq!(
        report.interests_storage_migration,
        MigrationDecision::NotRequired
    );
    assert_eq!(
        report.interests_revision_tracking_migration,
        MigrationDecision::Required
    );
}

#[rstest]
fn canonical_join_interests_with_revision_need_no_interests_migrations() {
    let report = UserStateSchemaAuditReport::evaluate(&canonical_join_interests_diagram(true));

    assert_eq!(
        report.interests_storage_coverage,
        InterestsStorageCoverage::CanonicalJoinTable
    );
    assert!(report.supports_interests_revision_tracking);
    assert_no_interests_migrations_required(&report);
}

#[rstest]
#[case(false, false)]
#[case(true, false)]
#[case(false, true)]
#[case(true, true)]
fn dual_model_interests_revision_tracking_follows_schema(
    #[case] preferences_has_revision: bool,
    #[case] join_has_revision: bool,
) {
    let report = UserStateSchemaAuditReport::evaluate(&dual_model_interests_diagram(
        preferences_has_revision,
        join_has_revision,
    ));

    assert_eq!(
        report.interests_storage_coverage,
        InterestsStorageCoverage::DualModel
    );
    let expects_revision_tracking = preferences_has_revision || join_has_revision;
    assert_eq!(
        report.supports_interests_revision_tracking,
        expects_revision_tracking
    );
    assert_eq!(
        report.interests_storage_migration,
        MigrationDecision::Required
    );
    assert_eq!(
        report.interests_revision_tracking_migration,
        if expects_revision_tracking {
            MigrationDecision::NotRequired
        } else {
            MigrationDecision::Required
        }
    );
    assert_eq!(
        report.update_conflict_handling_migration,
        if expects_revision_tracking {
            MigrationDecision::NotRequired
        } else {
            MigrationDecision::Required
        }
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

fn users_with_inline_credentials_diagram() -> SchemaDiagram {
    diagram(vec![table(
        "users",
        &["id", "display_name", "password_hash"],
    )])
}

fn separate_credentials_table_diagram() -> SchemaDiagram {
    diagram(vec![
        table("users", &["id", "display_name"]),
        table("user_credentials", &["user_id", "password_hash"]),
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

fn no_interests_diagram() -> SchemaDiagram {
    diagram(vec![table("users", &["id", "display_name"])])
}

fn canonical_join_interests_diagram(has_revision: bool) -> SchemaDiagram {
    let mut join_columns = vec!["user_id", "theme_id"];
    if has_revision {
        join_columns.push("revision");
    }

    diagram(vec![
        table("users", &["id", "display_name"]),
        table("user_interest_themes", join_columns.as_slice()),
    ])
}

fn dual_model_interests_diagram(
    preferences_has_revision: bool,
    join_has_revision: bool,
) -> SchemaDiagram {
    let mut preferences_columns = vec!["user_id", "interest_theme_ids"];
    if preferences_has_revision {
        preferences_columns.push("revision");
    }

    let mut join_columns = vec!["user_id", "theme_id"];
    if join_has_revision {
        join_columns.push("revision");
    }

    diagram(vec![
        table("users", &["id", "display_name"]),
        table("user_preferences", preferences_columns.as_slice()),
        table("user_interest_themes", join_columns.as_slice()),
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
