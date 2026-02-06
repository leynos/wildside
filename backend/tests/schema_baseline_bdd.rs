//! Behavioural tests for roadmap 3.1.1 schema baseline migration.

use backend::domain::ports::UserPersistenceError;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, Error as PostgresError, NoTls};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use uuid::Uuid;

#[path = "support/pg_embed.rs"]
mod pg_embed;

mod support;

use pg_embed::shared_cluster;
use support::{format_postgres_error, provision_template_database};

struct BaselineWorld {
    client: Client,
    _database: TemporaryDatabase,
    tables: Vec<String>,
    indexes: Vec<String>,
    last_error: Option<PostgresError>,
}

impl BaselineWorld {
    fn query_and_collect(&mut self, query: &str, target: &mut Vec<String>) {
        target.clear();
        let rows = self.client.query(query, &[]).expect("query rows");
        *target = rows.into_iter().map(|row| row.get(0)).collect();
    }

    fn query_table_names(&mut self) {
        let query = "SELECT tablename FROM pg_tables WHERE schemaname = 'public'";
        let mut tables = std::mem::take(&mut self.tables);
        self.query_and_collect(query, &mut tables);
        self.tables = tables;
    }

    fn query_indexes(&mut self) {
        let query = "SELECT indexdef FROM pg_indexes WHERE schemaname = 'public'";
        let mut indexes = std::mem::take(&mut self.indexes);
        self.query_and_collect(query, &mut indexes);
        self.indexes = indexes;
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "test helper signature is mandated by the migration test contract"
    )]
    fn insert_poi(
        &mut self,
        element_type: &str,
        id: i64,
        x: f64,
        y: f64,
    ) -> Result<u64, PostgresError> {
        self.client.execute(
            concat!(
                "INSERT INTO pois (element_type, id, location, osm_tags, narrative, popularity_score) ",
                "VALUES ($1, $2, point($3, $4), '{}'::jsonb, NULL, 0.0)"
            ),
            &[&element_type, &id, &x, &y],
        )
    }
}

#[fixture]
fn world() -> BaselineWorld {
    let cluster = shared_cluster().expect("embedded postgres cluster should be available");
    let database = provision_template_database(cluster)
        .map_err(|error: UserPersistenceError| error.to_string())
        .expect("template database should be provisioned");
    let client = Client::connect(database.url(), NoTls).expect("postgres client should connect");

    BaselineWorld {
        client,
        _database: database,
        tables: Vec::new(),
        indexes: Vec::new(),
        last_error: None,
    }
}

#[given("a migrated schema baseline")]
fn a_migrated_schema_baseline(world: &mut BaselineWorld) {
    let _ = world;
}

#[when("listing baseline tables")]
fn listing_baseline_tables(world: &mut BaselineWorld) {
    world.query_table_names();
}

#[then("all required baseline tables are present")]
fn all_required_baseline_tables_are_present(world: &mut BaselineWorld) {
    for required in [
        "interest_themes",
        "user_interest_themes",
        "pois",
        "poi_interest_themes",
        "route_pois",
        "route_summaries",
        "route_categories",
        "themes",
        "route_collections",
        "trending_route_highlights",
        "community_picks",
        "tags",
        "badges",
        "safety_toggles",
        "safety_presets",
    ] {
        assert!(
            world.tables.iter().any(|table| table == required),
            "missing expected table: {required}"
        );
    }
}

#[when("querying baseline indexes")]
fn querying_baseline_indexes(world: &mut BaselineWorld) {
    world.query_indexes();
}

#[then("GiST and GIN indexes are present")]
fn gist_and_gin_indexes_are_present(world: &mut BaselineWorld) {
    let index_blob = world.indexes.join("\n").to_lowercase();
    assert!(index_blob.contains("idx_pois_location_gist"));
    assert!(index_blob.contains("using gist (location"));
    assert!(index_blob.contains("idx_pois_osm_tags_gin"));
    assert!(index_blob.contains("using gin (osm_tags"));
}

#[given("a seeded route with two points of interest")]
fn a_seeded_route_with_two_points_of_interest(world: &mut BaselineWorld) {
    let user_id = Uuid::new_v4();
    let route_id = Uuid::new_v4();
    world
        .client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_id, &"Schema Baseline User"],
        )
        .expect("insert user");
    world
        .client
        .execute(
            concat!(
                "INSERT INTO routes (id, user_id, path, generation_params) VALUES ",
                "($1, $2, '((0,0),(1,1))'::path, '{}'::jsonb)"
            ),
            &[&route_id, &user_id],
        )
        .expect("insert route");
    world
        .client
        .execute(
            concat!(
                "INSERT INTO pois (element_type, id, location, osm_tags, narrative, popularity_score) ",
                "VALUES ('node', 1, point(0, 0), '{}'::jsonb, NULL, 0.0)"
            ),
            &[],
        )
        .expect("insert poi one");
    world
        .client
        .execute(
            concat!(
                "INSERT INTO pois (element_type, id, location, osm_tags, narrative, popularity_score) ",
                "VALUES ('node', 2, point(1, 1), '{}'::jsonb, NULL, 0.0)"
            ),
            &[],
        )
        .expect("insert poi two");
    world
        .client
        .execute(
            "INSERT INTO route_pois (route_id, poi_element_type, poi_id, position) VALUES ($1, 'node', 1, 0)",
            &[&route_id],
        )
        .expect("insert first route_poi");
}

#[when("inserting duplicate route positions")]
fn inserting_duplicate_route_positions(world: &mut BaselineWorld) {
    world.last_error = world
        .client
        .execute(
            "INSERT INTO route_pois (route_id, poi_element_type, poi_id, position) \
             SELECT route_id, 'node', 2, 0 FROM route_pois LIMIT 1",
            &[],
        )
        .err();
}

#[given("an existing point of interest")]
fn an_existing_point_of_interest(world: &mut BaselineWorld) {
    world
        .insert_poi("way", 42, 2.0, 2.0)
        .expect("insert baseline poi");
}

#[when("inserting a duplicate point of interest")]
fn inserting_a_duplicate_point_of_interest(world: &mut BaselineWorld) {
    world.last_error = world.insert_poi("way", 42, 2.0, 2.0).err();
}

#[then("insertion fails with a unique constraint violation")]
fn insertion_fails_with_a_unique_constraint_violation(world: &mut BaselineWorld) {
    let error = world
        .last_error
        .take()
        .expect("expected insert to fail with constraint error");
    let formatted = format_postgres_error(&error);
    assert!(
        formatted.to_lowercase().contains("duplicate key"),
        "expected duplicate-key error, got: {formatted}"
    );
}

#[scenario(
    path = "tests/features/schema_baseline.feature",
    name = "Baseline tables are materialised"
)]
fn baseline_tables_are_materialised(world: BaselineWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/schema_baseline.feature",
    name = "Spatial and JSON indexes are present"
)]
fn spatial_and_json_indexes_are_present(world: BaselineWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/schema_baseline.feature",
    name = "Duplicate route positions are rejected"
)]
fn duplicate_route_positions_are_rejected(world: BaselineWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/schema_baseline.feature",
    name = "Duplicate POI composite keys are rejected"
)]
fn duplicate_poi_composite_keys_are_rejected(world: BaselineWorld) {
    let _ = world;
}
