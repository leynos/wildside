//! Behavioural tests for example-data crate.
//!
//! These tests validate the crate's behaviour against Gherkin scenarios
//! covering registry parsing, deterministic generation, and validation.

// `expect` is idiomatic in test code for failing fast on precondition violations.
#![expect(
    clippy::expect_used,
    reason = "test code uses expect for clear failure messages"
)]

use std::collections::HashSet;

use example_data::{
    ExampleUserSeed, RegistryError, SeedDefinition, SeedRegistry, generate_example_users,
    is_valid_display_name,
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

// ============================================================================
// Test fixtures and constants
// ============================================================================

/// Base valid registry JSON used by multiple Given steps.
const VALID_REGISTRY_JSON: &str = r#"{
    "version": 1,
    "interestThemeIds": [
        "3fa85f64-5717-4562-b3fc-2c963f66afa6",
        "4fa85f64-5717-4562-b3fc-2c963f66afa7"
    ],
    "safetyToggleIds": [
        "7fa85f64-5717-4562-b3fc-2c963f66afa6"
    ],
    "seeds": [
        {"name": "test-seed", "seed": 42, "userCount": 5}
    ]
}"#;

/// Test world holding parsed registry and generated users.
#[derive(Default, ScenarioState)]
struct World {
    json_input: Slot<String>,
    registry_result: Slot<Result<SeedRegistry, RegistryError>>,
    seed_def: Slot<SeedDefinition>,
    generated_users: Slot<Vec<ExampleUserSeed>>,
    second_generation: Slot<Vec<ExampleUserSeed>>,
}

impl World {
    /// Extracts the valid registry from the world state.
    fn registry(&self) -> SeedRegistry {
        self.registry_result
            .get()
            .expect("registry should be set")
            .expect("registry should be valid")
    }

    /// Extracts the seed definition from the world state.
    fn seed_def(&self) -> SeedDefinition {
        self.seed_def.get().expect("seed definition should be set")
    }

    /// Extracts the registry result (Ok or Err) from the world state.
    fn registry_result(&self) -> Result<SeedRegistry, RegistryError> {
        self.registry_result
            .get()
            .expect("registry result should be set")
    }

    /// Extracts the generated users from the world state.
    fn users(&self) -> Vec<ExampleUserSeed> {
        self.generated_users
            .get()
            .expect("users should be generated")
    }
}

#[fixture]
fn world() -> World {
    World::default()
}

// ============================================================================
// Given steps
// ============================================================================

#[given("a valid seed registry JSON")]
fn a_valid_seed_registry_json(world: &World) {
    world.json_input.set(VALID_REGISTRY_JSON.to_owned());
}

#[given("a valid seed registry")]
fn a_valid_seed_registry(world: &World) {
    let registry = SeedRegistry::from_json(VALID_REGISTRY_JSON).expect("valid test registry");
    world.registry_result.set(Ok(registry));
}

#[given("a valid seed registry with interest theme IDs")]
fn a_valid_seed_registry_with_interest_theme_ids(world: &World) {
    let json = r#"{
        "version": 1,
        "interestThemeIds": [
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "4fa85f64-5717-4562-b3fc-2c963f66afa7",
            "5fa85f64-5717-4562-b3fc-2c963f66afa8"
        ],
        "safetyToggleIds": [],
        "seeds": [
            {"name": "test-seed", "seed": 42, "userCount": 10}
        ]
    }"#;
    let registry = SeedRegistry::from_json(json).expect("valid test registry");
    world.registry_result.set(Ok(registry));
}

#[given("a seed definition with seed {seed:u64}")]
fn a_seed_definition_with_seed(world: &World, seed: u64) {
    let registry = world.registry();
    let seed_def = registry.find_seed("test-seed").expect("seed exists");
    assert_eq!(seed_def.seed(), seed, "seed value mismatch");
    world.seed_def.set(seed_def.clone());
}

#[given("a seed definition")]
fn a_seed_definition(world: &World) {
    let registry = world.registry();
    let seed_def = registry.find_seed("test-seed").expect("seed exists");
    world.seed_def.set(seed_def.clone());
}

#[given("malformed JSON")]
fn malformed_json(world: &World) {
    world.json_input.set("not valid json".to_owned());
}

#[given("registry JSON with empty seeds array")]
fn registry_json_with_empty_seeds_array(world: &World) {
    let json = r#"{
        "version": 1,
        "interestThemeIds": [],
        "safetyToggleIds": [],
        "seeds": []
    }"#;
    world.json_input.set(json.to_owned());
}

#[given("registry JSON with invalid interest theme UUID")]
fn registry_json_with_invalid_interest_theme_uuid(world: &World) {
    let json = r#"{
        "version": 1,
        "interestThemeIds": ["not-a-uuid"],
        "safetyToggleIds": [],
        "seeds": [{"name": "test", "seed": 1, "userCount": 1}]
    }"#;
    world.json_input.set(json.to_owned());
}

// ============================================================================
// When steps
// ============================================================================

#[when("the registry is parsed")]
fn the_registry_is_parsed(world: &World) {
    let json_opt = world.json_input.get();
    let json = json_opt.expect("JSON input should be set");
    let result = SeedRegistry::from_json(&json);
    world.registry_result.set(result);
}

#[when("users are generated")]
fn users_are_generated(world: &World) {
    let registry = world.registry();
    let seed_def = world.seed_def();
    let users = generate_example_users(&registry, &seed_def).expect("generation succeeds");
    world.generated_users.set(users);
}

#[when("users are generated twice")]
fn users_are_generated_twice(world: &World) {
    let registry = world.registry();
    let seed_def = world.seed_def();

    let first = generate_example_users(&registry, &seed_def).expect("first generation");
    let second = generate_example_users(&registry, &seed_def).expect("second generation");

    world.generated_users.set(first);
    world.second_generation.set(second);
}

// ============================================================================
// Then steps
// ============================================================================

#[then("parsing succeeds")]
fn parsing_succeeds(world: &World) {
    let result = world.registry_result();
    assert!(result.is_ok(), "Expected parsing to succeed: {result:?}");
}

#[then("the registry contains the expected seed definitions")]
fn the_registry_contains_the_expected_seed_definitions(world: &World) {
    let registry = world.registry();
    assert_eq!(registry.seeds().len(), 1);
    let seed = registry.find_seed("test-seed").expect("seed should exist");
    assert_eq!(seed.name(), "test-seed");
    assert_eq!(seed.seed(), 42);
    assert_eq!(seed.user_count(), 5);
}

#[then("both generations produce identical users")]
fn both_generations_produce_identical_users(world: &World) {
    let first_opt = world.generated_users.get();
    let first = first_opt.expect("first generation should be set");
    let second_opt = world.second_generation.get();
    let second = second_opt.expect("second generation should be set");

    assert_eq!(first, second, "Generations should be deterministic");
}

#[then("all display names satisfy backend constraints")]
fn all_display_names_satisfy_backend_constraints(world: &World) {
    for user in world.users() {
        assert!(
            is_valid_display_name(&user.display_name),
            "Invalid display name: {}",
            user.display_name
        );
    }
}

#[then("all interest theme IDs exist in the registry")]
fn all_interest_theme_ids_exist_in_the_registry(world: &World) {
    let registry = world.registry();
    let registry_ids: HashSet<_> = registry.interest_theme_ids().iter().collect();

    for user in world.users() {
        for id in &user.interest_theme_ids {
            assert!(
                registry_ids.contains(id),
                "Interest theme {id} not in registry"
            );
        }
    }
}

#[then("parsing fails with a parse error")]
fn parsing_fails_with_a_parse_error(world: &World) {
    match world.registry_result() {
        Err(RegistryError::ParseError { .. }) => {}
        other => panic!("Expected ParseError, got: {other:?}"),
    }
}

#[then("parsing fails with empty seeds error")]
fn parsing_fails_with_empty_seeds_error(world: &World) {
    match world.registry_result() {
        Err(RegistryError::EmptySeeds) => {}
        other => panic!("Expected EmptySeeds, got: {other:?}"),
    }
}

#[then("parsing fails with invalid UUID error")]
fn parsing_fails_with_invalid_uuid_error(world: &World) {
    match world.registry_result() {
        Err(RegistryError::InvalidInterestThemeId { .. }) => {}
        other => panic!("Expected InvalidInterestThemeId, got: {other:?}"),
    }
}

// ============================================================================
// Scenario bindings
// ============================================================================

#[scenario(
    path = "tests/features/example_data.feature",
    name = "Valid registry parses successfully"
)]
fn valid_registry_parses_successfully(world: World) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data.feature",
    name = "Deterministic generation produces identical users"
)]
fn deterministic_generation_produces_identical_users(world: World) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data.feature",
    name = "Generated display names are valid"
)]
fn generated_display_names_are_valid(world: World) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data.feature",
    name = "Interest theme selection stays within registry"
)]
fn interest_theme_selection_stays_within_registry(world: World) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data.feature",
    name = "Invalid JSON fails parsing"
)]
fn invalid_json_fails_parsing(world: World) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data.feature",
    name = "Empty seeds array fails parsing"
)]
fn empty_seeds_array_fails_parsing(world: World) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data.feature",
    name = "Invalid UUID in interest themes fails parsing"
)]
fn invalid_uuid_in_interest_themes_fails_parsing(world: World) {
    let _ = world;
}
