//! Static contract checks for roadmap 3.1.1 migration SQL.

use rstest::rstest;

const MIGRATION_UP: &str = include_str!(
    "../migrations/2026-02-06-012424_schema_baseline_catalogue_descriptor_user_state/up.sql"
);
const MIGRATION_DOWN: &str = include_str!(
    "../migrations/2026-02-06-012424_schema_baseline_catalogue_descriptor_user_state/down.sql"
);

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
fn creates_expected_spatial_and_json_indexes() {
    // These static SQL checks intentionally assert specific strings; if the
    // migration DDL is reformatted, update these literals alongside it.
    assert!(MIGRATION_UP.contains("idx_pois_location_gist"));
    assert!(MIGRATION_UP.contains("USING GIST (location)"));
    assert!(MIGRATION_UP.contains("idx_pois_osm_tags_gin"));
    assert!(MIGRATION_UP.contains("USING GIN (osm_tags)"));
}

#[rstest]
fn enforces_composite_and_positional_constraints() {
    assert!(MIGRATION_UP.contains("PRIMARY KEY (element_type, id)"));
    assert!(MIGRATION_UP.contains("PRIMARY KEY (route_id, poi_element_type, poi_id)"));
    assert!(MIGRATION_UP.contains("UNIQUE (route_id, position)"));
}

#[rstest]
fn down_migration_restores_route_compatibility_columns() {
    assert!(MIGRATION_DOWN.contains("ADD COLUMN IF NOT EXISTS request_id UUID"));
    assert!(MIGRATION_DOWN.contains("ADD COLUMN IF NOT EXISTS plan_snapshot JSONB"));
    assert!(MIGRATION_DOWN.contains("CREATE INDEX IF NOT EXISTS idx_routes_request_id"));
    assert!(MIGRATION_DOWN.contains("CREATE TRIGGER update_routes_updated_at"));
}
