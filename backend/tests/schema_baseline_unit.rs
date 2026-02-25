//! Static contract checks for roadmap 3.1.1 and 3.4.1 migration SQL.

use rstest::rstest;

const MIGRATION_UP: &str = include_str!(
    "../migrations/2026-02-06-012424_schema_baseline_catalogue_descriptor_user_state/up.sql"
);
const MIGRATION_DOWN: &str = include_str!(
    "../migrations/2026-02-06-012424_schema_baseline_catalogue_descriptor_user_state/down.sql"
);
const OFFLINE_WALK_UP: &str =
    include_str!("../migrations/2026-02-20-000000_create_offline_bundles_and_walk_sessions/up.sql");
const OFFLINE_WALK_DOWN: &str = include_str!(
    "../migrations/2026-02-20-000000_create_offline_bundles_and_walk_sessions/down.sql"
);
const OSM_PROVENANCE_UP: &str =
    include_str!("../migrations/2026-02-24-000000_create_osm_ingestion_provenance/up.sql");
const OSM_PROVENANCE_DOWN: &str =
    include_str!("../migrations/2026-02-24-000000_create_osm_ingestion_provenance/down.sql");

#[rstest]
fn enables_required_extensions() {
    assert!(MIGRATION_UP.contains("CREATE EXTENSION IF NOT EXISTS pgcrypto;"));
}

#[rstest]
#[case("CREATE TABLE IF NOT EXISTS interest_themes")]
#[case("CREATE TABLE IF NOT EXISTS user_interest_themes")]
#[case("CREATE TABLE IF NOT EXISTS pois")]
#[case("CREATE TABLE IF NOT EXISTS poi_interest_themes")]
#[case("CREATE TABLE IF NOT EXISTS route_pois")]
#[case("CREATE TABLE IF NOT EXISTS route_summaries")]
#[case("CREATE TABLE IF NOT EXISTS route_categories")]
#[case("CREATE TABLE IF NOT EXISTS themes")]
#[case("CREATE TABLE IF NOT EXISTS route_collections")]
#[case("CREATE TABLE IF NOT EXISTS trending_route_highlights")]
#[case("CREATE TABLE IF NOT EXISTS community_picks")]
#[case("CREATE TABLE IF NOT EXISTS tags")]
#[case("CREATE TABLE IF NOT EXISTS badges")]
#[case("CREATE TABLE IF NOT EXISTS safety_toggles")]
#[case("CREATE TABLE IF NOT EXISTS safety_presets")]
fn creates_expected_baseline_tables(#[case] table_ddl: &str) {
    assert!(
        MIGRATION_UP.contains(table_ddl),
        "expected migration to contain: {table_ddl}"
    );
}

#[rstest]
#[case("idx_pois_location_gist")]
#[case("USING GIST (location)")]
#[case("idx_pois_osm_tags_gin")]
#[case("USING GIN (osm_tags)")]
fn creates_expected_spatial_and_json_indexes(#[case] index_fragment: &str) {
    // These static SQL checks intentionally assert specific strings; if the
    // migration DDL is reformatted, update these literals alongside it.
    assert!(
        MIGRATION_UP.contains(index_fragment),
        "expected migration to contain index fragment: {index_fragment}"
    );
}

#[rstest]
fn enforces_composite_and_positional_constraints() {
    assert!(MIGRATION_UP.contains("PRIMARY KEY (element_type, id)"));
    assert!(MIGRATION_UP.contains("PRIMARY KEY (route_id, poi_element_type, poi_id)"));
    assert!(MIGRATION_UP.contains("UNIQUE (route_id, position)"));
}

#[rstest]
#[case("ADD COLUMN IF NOT EXISTS request_id UUID")]
#[case("ADD COLUMN IF NOT EXISTS plan_snapshot JSONB")]
#[case("CREATE INDEX IF NOT EXISTS idx_routes_request_id")]
#[case("CREATE TRIGGER update_routes_updated_at")]
fn down_migration_restores_route_compatibility_columns(#[case] ddl_fragment: &str) {
    assert!(
        MIGRATION_DOWN.contains(ddl_fragment),
        "expected down migration to contain: {ddl_fragment}"
    );
}

#[rstest]
#[case("CREATE TABLE offline_bundles")]
#[case("CREATE TABLE walk_sessions")]
fn creates_offline_and_walk_tables(#[case] table_ddl: &str) {
    assert!(
        OFFLINE_WALK_UP.contains(table_ddl),
        "expected migration to contain: {table_ddl}"
    );
}

#[rstest]
#[case("offline_bundles_bounds_valid")]
#[case("offline_bundles_kind_reference_valid")]
#[case("offline_bundles_status_progress_valid")]
#[case("bounds[1] IS NOT NULL")]
#[case("idx_offline_bundles_owner_device_created_at")]
#[case("idx_offline_bundles_anonymous_device_created_at")]
#[case("CREATE TRIGGER update_offline_bundles_updated_at")]
fn enforces_offline_bundle_constraints_and_indexes(#[case] constraint_or_index: &str) {
    assert!(
        OFFLINE_WALK_UP.contains(constraint_or_index),
        "expected offline bundle migration to contain: {constraint_or_index}"
    );
}

#[rstest]
#[case("walk_sessions_ended_after_started")]
#[case("walk_sessions_updated_after_created")]
#[case("idx_walk_sessions_user_completed_ended_at_desc")]
#[case("CREATE TRIGGER update_walk_sessions_updated_at")]
fn enforces_walk_summary_query_support_and_audit_constraints(#[case] constraint_or_index: &str) {
    assert!(
        OFFLINE_WALK_UP.contains(constraint_or_index),
        "expected walk session migration to contain: {constraint_or_index}"
    );
}

#[rstest]
#[case("DROP TABLE IF EXISTS walk_sessions")]
#[case("DROP TABLE IF EXISTS offline_bundles")]
#[case("DROP TRIGGER IF EXISTS update_walk_sessions_updated_at")]
#[case("DROP TRIGGER IF EXISTS update_offline_bundles_updated_at")]
fn offline_walk_down_migration_drops_schema_objects(#[case] drop_statement: &str) {
    assert!(
        OFFLINE_WALK_DOWN.contains(drop_statement),
        "expected down migration to contain drop statement: {drop_statement}"
    );
}

#[rstest]
#[case("CREATE TABLE IF NOT EXISTS osm_ingestion_provenance")]
#[case("geofence_id TEXT NOT NULL")]
#[case("input_digest TEXT NOT NULL")]
#[case("raw_poi_count BIGINT NOT NULL CHECK (raw_poi_count >= 0)")]
#[case("filtered_poi_count BIGINT NOT NULL CHECK (filtered_poi_count >= 0)")]
#[case("CONSTRAINT osm_ingestion_provenance_rerun_unique UNIQUE (geofence_id, input_digest)")]
#[case("osm_ingestion_provenance_rerun_unique")]
#[case("ON osm_ingestion_provenance (geofence_id, imported_at DESC)")]
fn creates_osm_ingestion_provenance_contract(#[case] ddl_fragment: &str) {
    assert!(
        OSM_PROVENANCE_UP.contains(ddl_fragment),
        "expected OSM provenance migration to contain: {ddl_fragment}"
    );
}

#[rstest]
#[case("DROP INDEX IF EXISTS idx_osm_ingestion_provenance_geofence_imported_at")]
#[case("DROP TABLE IF EXISTS osm_ingestion_provenance")]
fn osm_ingestion_provenance_down_migration_drops_schema_objects(#[case] drop_statement: &str) {
    assert!(
        OSM_PROVENANCE_DOWN.contains(drop_statement),
        "expected OSM provenance down migration to contain: {drop_statement}"
    );
}
