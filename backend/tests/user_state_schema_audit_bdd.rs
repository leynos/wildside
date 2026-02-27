//! Behavioural tests for roadmap 3.5.1 user-state schema audits.

use backend::domain::{
    EntitySchemaCoverage, InterestsStorageCoverage, LoginSchemaCoverage, MigrationDecision,
    UserStateSchemaAuditReport, audit_user_state_schema_coverage,
};
use backend::outbound::persistence::PostgresSchemaSnapshotRepository;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

mod support;

use support::atexit_cleanup::shared_cluster_handle;
use support::embedded_postgres::drop_users_table;
use support::{drop_table, handle_cluster_setup_failure, provision_template_database};

#[derive(Debug)]
struct UserStateSchemaAuditWorld {
    database: Option<TemporaryDatabase>,
    report: Option<UserStateSchemaAuditReport>,
    setup_error: Option<String>,
}

impl UserStateSchemaAuditWorld {
    fn ready(database: TemporaryDatabase) -> Self {
        Self {
            database: Some(database),
            report: None,
            setup_error: None,
        }
    }

    fn skipped(reason: String) -> Self {
        Self {
            database: None,
            report: None,
            setup_error: Some(reason),
        }
    }

    fn is_skipped(&self) -> bool {
        self.setup_error.is_some()
    }

    fn database_url(&self) -> &str {
        self.database
            .as_ref()
            .expect("database should be available")
            .url()
    }
}

fn skip_if_needed(world: &UserStateSchemaAuditWorld) -> bool {
    if world.is_skipped() {
        let reason = world.setup_error.as_deref().unwrap_or("unknown reason");
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped ({reason})");
        true
    } else {
        false
    }
}

fn get_report_or_skip(world: &UserStateSchemaAuditWorld) -> Option<&UserStateSchemaAuditReport> {
    if skip_if_needed(world) {
        return None;
    }
    Some(world.report.as_ref().expect("report should be captured"))
}

#[fixture]
fn world() -> UserStateSchemaAuditWorld {
    let cluster = match shared_cluster_handle() {
        Ok(cluster) => cluster,
        Err(reason) => {
            let message = reason.to_string();
            let _: Option<()> = handle_cluster_setup_failure(&message);
            return UserStateSchemaAuditWorld::skipped(message);
        }
    };

    let database = match provision_template_database(cluster).map_err(|error| error.to_string()) {
        Ok(database) => database,
        Err(reason) => {
            let _: Option<()> = handle_cluster_setup_failure(reason.clone());
            return UserStateSchemaAuditWorld::skipped(reason);
        }
    };

    UserStateSchemaAuditWorld::ready(database)
}

#[given("a migrated schema baseline")]
fn a_migrated_schema_baseline(world: &mut UserStateSchemaAuditWorld) {
    let _ = world;
}

#[given("the users table is missing")]
fn the_users_table_is_missing(world: &mut UserStateSchemaAuditWorld) {
    if skip_if_needed(world) {
        return;
    }

    drop_users_table(world.database_url()).expect("users table should drop");
}

#[given("interests use a canonical revisioned model")]
fn interests_use_a_canonical_revisioned_model(world: &mut UserStateSchemaAuditWorld) {
    if skip_if_needed(world) {
        return;
    }

    drop_table(world.database_url(), "user_interest_themes")
        .expect("join-table interests model should be removable");
}

#[when("executing the user state schema audit")]
fn executing_the_user_state_schema_audit(world: &mut UserStateSchemaAuditWorld) {
    if skip_if_needed(world) {
        return;
    }

    let repository = PostgresSchemaSnapshotRepository::new(world.database_url());
    world.report =
        Some(audit_user_state_schema_coverage(&repository).expect("schema audit should succeed"));
}

#[then("login credentials storage is reported as missing")]
fn login_credentials_storage_is_reported_as_missing(world: &mut UserStateSchemaAuditWorld) {
    let Some(report) = get_report_or_skip(world) else {
        return;
    };
    assert_eq!(
        report.login_coverage,
        LoginSchemaCoverage::MissingCredentialStorage
    );
}

#[then("users and profile storage are reported as covered")]
fn users_and_profile_storage_are_reported_as_covered(world: &mut UserStateSchemaAuditWorld) {
    let Some(report) = get_report_or_skip(world) else {
        return;
    };
    assert_eq!(report.users_coverage, EntitySchemaCoverage::Covered);
    assert_eq!(report.profile_coverage, EntitySchemaCoverage::Covered);
    assert_eq!(
        report.profile_storage_migration,
        MigrationDecision::NotRequired
    );
}

#[then("interests migration decisions are required")]
fn interests_migration_decisions_are_required(world: &mut UserStateSchemaAuditWorld) {
    let Some(report) = get_report_or_skip(world) else {
        return;
    };
    assert_eq!(
        report.interests_storage_coverage,
        InterestsStorageCoverage::DualModel
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

#[then("users and profile migrations are required")]
fn users_and_profile_migrations_are_required(world: &mut UserStateSchemaAuditWorld) {
    let Some(report) = get_report_or_skip(world) else {
        return;
    };
    assert_eq!(report.users_coverage, EntitySchemaCoverage::Missing);
    assert_eq!(report.profile_coverage, EntitySchemaCoverage::Missing);
    assert_eq!(
        report.profile_storage_migration,
        MigrationDecision::Required
    );
}

#[then("interests migration decisions are not required")]
fn interests_migration_decisions_are_not_required(world: &mut UserStateSchemaAuditWorld) {
    let Some(report) = get_report_or_skip(world) else {
        return;
    };
    assert_eq!(
        report.interests_storage_coverage,
        InterestsStorageCoverage::CanonicalPreferences
    );
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

#[scenario(
    path = "tests/features/user_state_schema_audit.feature",
    name = "Baseline schema reports user-state migration gaps"
)]
fn baseline_schema_reports_user_state_migration_gaps(world: UserStateSchemaAuditWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_state_schema_audit.feature",
    name = "Missing users table requires users and profile migrations"
)]
fn missing_users_table_requires_users_and_profile_migrations(world: UserStateSchemaAuditWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_state_schema_audit.feature",
    name = "Canonical interests model with revision needs no interests migrations"
)]
fn canonical_interests_model_with_revision_needs_no_interests_migrations(
    world: UserStateSchemaAuditWorld,
) {
    drop(world);
}
