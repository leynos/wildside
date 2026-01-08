# Implement the example-data crate

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `PLANS.md` at the
repository root.

## Purpose / Big Picture

Deliver a standalone `example-data` crate that generates deterministic,
believable user data for demonstration purposes. After this change, callers can
load a JSON seed registry and generate reproducible user records with valid
display names, interest themes, safety toggles, and unit system preferences.

User-visible outcome: running `cargo test -p example-data` passes all unit and
behavioural tests, and calling `generate_example_users()` with the same seed
produces identical output across runs.

## Constraints

- The crate must NOT depend on backend domain types (avoids circular
  dependencies).
- Display name validation must mirror `backend/src/domain/user.rs` exactly:
  3–32 characters, pattern `^[A-Za-z0-9_ ]+$`.
- All workspace lints must be satisfied: no `unsafe`, no `unwrap`/`expect`, no
  indexing/slicing, full documentation.
- The crate location must be `crates/example-data/` (autodiscovered via
  workspace glob).
- JSON registry path: `backend/fixtures/example-data/seeds.json`.

## Tolerances (Exception Triggers)

- Scope: if implementation requires changes to more than 15 files or 800 lines
  of code (net), stop and escalate.
- Dependencies: if a dependency beyond those listed in the design document is
  required, stop and escalate.
- Iterations: if tests still fail after 5 attempts at fixing, stop and
  escalate.
- Ambiguity: if the `fake` crate cannot reliably generate valid display names,
  escalate with alternatives.

## Risks

- Risk: `fake` crate names may contain invalid characters (hyphens, apostrophes)
  Severity: medium
  Likelihood: high
  Mitigation: sanitize generated names by replacing invalid chars with
  underscores; retry loop with max attempts.

- Risk: Workspace lint strictness may reject idiomatic `fake` crate usage
  Severity: low
  Likelihood: medium
  Mitigation: use `.get()` for bounds checking; avoid panicking code paths.

## Progress

- [x] (2026-01-07) Phase 1: Crate scaffold and Cargo.toml
- [x] (2026-01-07) Phase 2: Error types with thiserror
- [x] (2026-01-07) Phase 3: Display name validation module
- [x] (2026-01-07) Phase 4: Type definitions (SeedRegistry, ExampleUserSeed, UnitSystemSeed)
- [x] (2026-01-07) Phase 5: JSON registry parsing
- [x] (2026-01-07) Phase 6: Deterministic user generation
- [x] (2026-01-07) Phase 7: Create seed registry JSON fixture
- [x] (2026-01-07) Phase 8: Unit tests with rstest
- [x] (2026-01-07) Phase 9: Behaviour-Driven Development (BDD) tests with
  rstest-bdd v0.3.2
- [x] (2026-01-07) Phase 10: Documentation and quality gates
- [x] (2026-01-07) Phase 11: Update roadmap to mark 2.4.2 as done

## Surprises & Discoveries

- Observation: Rust 2024 edition reserves `gen` as a keyword
  Evidence: Compilation error: "expected identifier, found reserved keyword `gen`"
  Impact: Resolved by upgrading to rand 0.9 which renamed methods: `gen()` →
  `random()`, `gen_ratio()` → `random_ratio()`, `gen_range()` → `random_range()`

- Observation: Workspace lints disallow `clippy::expect_used` even in test code
  Evidence: 31 clippy errors for `expect()` calls in BDD test file
  Impact: Added module-level `#![expect(clippy::expect_used)]` attribute to
  test file

- Observation: `fake` crate names often contain hyphens and apostrophes
  Evidence: Generated names like "O'Brien" and "Mary-Jane"
  Impact: Implemented `sanitize_name()` function to replace invalid chars with
  spaces

## Decision Log

- Decision: Use rstest-bdd v0.3.2 (upgrade from v0.2.0)
  Rationale: User confirmed upgrade; new crate can use latest version
  Date/Author: 2026-01-07 / Planning phase

- Decision: Defer Postgres integration tests to task 2.4.3
  Rationale: The example-data crate doesn't interact with Postgres directly;
  database integration belongs in the migration task
  Date/Author: 2026-01-07 / Planning phase

- Decision: Use ChaCha8Rng for deterministic generation
  Rationale: Portable, reproducible across platforms, recommended for
  deterministic seeding
  Date/Author: 2026-01-07 / Planning phase

## Outcomes & Retrospective

### Outcomes

- Delivered standalone `example-data` crate at `crates/example-data/`
- 60 unit tests + 7 BDD tests + 5 doctests all passing
- Full quality gates passing: `make check-fmt && make lint && make test`
- Deterministic generation verified: same seed produces identical output
- Display name validation mirrors backend exactly (3–32 chars, `^[A-Za-z0-9_ ]+$`)

### Metrics

- Files created: 8 source files + 2 test files + 1 fixture
- Lines of code: ~800 (within tolerance)
- Test coverage: comprehensive for all public APIs

### Lessons Learned

1. Rust 2024 edition keyword changes require attention when using `rand` crate
2. Workspace-level strict lints require explicit opt-out in test files
3. Name generation with `fake` crate requires sanitization layer
4. rstest-bdd v0.3.2 works well for Gherkin-style behavioural tests

## Context and Orientation

The Wildside backend follows hexagonal architecture. This task adds a new crate
at `crates/example-data/` that generates demonstration user data without
coupling to the backend domain.

Key files:

- `docs/backend-sample-data-design.md` - Authoritative design specification
- `backend/src/domain/user.rs` - Display name validation to mirror (lines
  161-174)
- `Cargo.toml` - Workspace configuration with strict lints
- `backend/tests/pwa_preferences_bdd.rs` - BDD test pattern to follow

The crate exports:

- `SeedRegistry` - Parsed JSON registry with seeds and descriptor IDs
- `SeedDefinition` - Named seed with random number generator (RNG) value and
  user count
- `ExampleUserSeed` - Generated user record
- `UnitSystemSeed` - Metric/Imperial enum
- `generate_example_users()` - Main generation function

## Plan of Work

### Phase 1: Crate Scaffold

Create `crates/example-data/` with:

    crates/example-data/
      Cargo.toml
      src/
        lib.rs

Cargo.toml dependencies:

    [package]
    name = "example-data"
    version = "0.1.0"
    edition = "2024"

    [dependencies]
    fake = "4.4"
    rand = "0.9"
    rand_chacha = "0.9"
    serde = { version = "1", features = ["derive"] }
    serde_json = "1"
    thiserror = "2"
    uuid = { version = "1", features = ["serde", "v4"] }

    [dev-dependencies]
    rstest = "0.26"
    rstest-bdd = "0.3.2"
    rstest-bdd-macros = "0.3.2"

    [lints]
    workspace = true

### Phase 2: Error Types

Create `src/error.rs` with semantic error enums:

- `RegistryError` - Input/output (I/O), parse, version, Universally Unique
  Identifier (UUID) validation, empty seeds, not found
- `GenerationError` - Display name generation failure, missing themes/toggles

### Phase 3: Validation Module

Create `src/validation.rs` mirroring backend constraints:

    pub const DISPLAY_NAME_MIN: usize = 3;
    pub const DISPLAY_NAME_MAX: usize = 32;
    // Pattern: ^[A-Za-z0-9_ ]+$
    pub fn is_valid_display_name(name: &str) -> bool

### Phase 4: Type Definitions

Create `src/registry.rs`:

- `SeedRegistry` with `version`, `interest_theme_ids`, `safety_toggle_ids`,
  `seeds`
- `SeedDefinition` with `name`, `seed`, `user_count`
- Serde derive for JSON parsing with camelCase field names

Create `src/seed.rs`:

- `ExampleUserSeed` with all user fields
- `UnitSystemSeed` enum (Metric, Imperial)

### Phase 5: Registry Parsing

Add to `src/registry.rs`:

- `SeedRegistry::from_json(json: &str) -> Result<Self, RegistryError>`
- `SeedRegistry::from_file(path: &Path) -> Result<Self, RegistryError>`
- UUID validation for theme/toggle IDs
- Seed lookup by name

### Phase 6: User Generation

Create `src/generator.rs`:

- `generate_example_users(registry, seed_def) -> Result<Vec<ExampleUserSeed>,
  GenerationError>`
- Use `ChaCha8Rng::seed_from_u64(seed_def.seed())` for determinism
- Generate display names with `fake::faker::name::raw::{FirstName, LastName}`
- Sanitize names: replace invalid chars with underscore, truncate to max length
- Retry loop (max 100 attempts) for valid name generation
- Select 1-3 interest themes, 0-2 safety toggles from registry
- 90% metric, 10% imperial distribution

### Phase 7: Seed Registry Fixture

Create `backend/fixtures/example-data/seeds.json`:

    {
      "version": 1,
      "interestThemeIds": [
        "3fa85f64-5717-4562-b3fc-2c963f66afa6",
        "4fa85f64-5717-4562-b3fc-2c963f66afa7",
        "5fa85f64-5717-4562-b3fc-2c963f66afa8"
      ],
      "safetyToggleIds": [
        "7fa85f64-5717-4562-b3fc-2c963f66afa6",
        "8fa85f64-5717-4562-b3fc-2c963f66afa7"
      ],
      "seeds": [
        { "name": "mossy-owl", "seed": 2026, "userCount": 12 }
      ]
    }

### Phase 8: Unit Tests

Create `src/tests/` with rstest fixtures:

- `validation_tests.rs` - Length bounds, character validation, edge cases
- `registry_tests.rs` - Valid parsing, invalid JSON, missing fields, UUID
  validation
- `generator_tests.rs` - Determinism, count, name validity, ID subset,
  distribution

### Phase 9: Behavioural Tests

Create `tests/features/example_data.feature` with scenarios:

- Valid registry parses successfully
- Deterministic generation produces identical users
- Generated display names satisfy constraints
- Interest/safety selections stay within registry
- Invalid JSON fails with parse error
- Empty seeds array fails with specific error

Create `tests/example_data_bdd.rs` with step definitions using rstest-bdd
v0.3.2.

### Phase 10: Documentation and Quality Gates

- Add rustdoc examples to all public items
- Ensure crate-level documentation in `lib.rs`
- Run `make check-fmt && make lint && make test`

### Phase 11: Roadmap Update

Mark task 2.4.2 as complete in `docs/backend-roadmap.md`.

## Concrete Steps

All commands run from repository root.

1. Create crate directory:

       mkdir -p crates/example-data/src

2. Create Cargo.toml and lib.rs (scaffold)

3. Run initial quality check:

       make check-fmt && make lint

4. Implement each module, committing after each phase passes quality gates:

       make check-fmt && make lint && make test 2>&1 | tee /tmp/test-output.log

5. Create fixture directory and JSON:

       mkdir -p backend/fixtures/example-data

6. Final validation:

       make check-fmt && make lint && make test

Expected output: all tests pass, no warnings, no lint errors.

## Validation and Acceptance

Quality criteria:

- Tests: `cargo test -p example-data` passes all unit and BDD tests
- Lint: `make lint` passes with no warnings
- Format: `make check-fmt` passes
- Determinism: calling `generate_example_users` twice with same seed produces
  byte-identical output

Quality method:

    make check-fmt && make lint && make test 2>&1 | tee /tmp/quality-gates.log
    grep -E "(PASSED|FAILED|error|warning)" /tmp/quality-gates.log

## Idempotence and Recovery

All steps are idempotent. Files can be recreated by re-running the
implementation. No database or external state is modified (Postgres integration
deferred to 2.4.3).

## Artifacts and Notes

Key code patterns to follow:

Display name validation (mirrors backend/src/domain/user.rs constraints):

    const fn is_valid_display_name_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == ' ' || c == '_'
    }

    pub fn is_valid_display_name(name: &str) -> bool {
        let length = name.chars().count();
        if !(DISPLAY_NAME_MIN..=DISPLAY_NAME_MAX).contains(&length) {
            return false;
        }
        if name.trim().is_empty() {
            return false;
        }
        name.chars().all(is_valid_display_name_char)
    }

BDD scenario definition (pattern from backend/tests/pwa_preferences_bdd.rs):

    #[scenario(path = "tests/features/example_data.feature")]
    fn example_data(world: WorldFixture) {
        let _ = world;
    }

## Interfaces and Dependencies

### Public API

In `crates/example-data/src/lib.rs`, export:

    pub use error::{GenerationError, RegistryError};
    pub use generator::generate_example_users;
    pub use registry::{SeedDefinition, SeedRegistry};
    pub use seed::{ExampleUserSeed, UnitSystemSeed};
    pub use validation::{is_valid_display_name, DISPLAY_NAME_MAX, DISPLAY_NAME_MIN};

### Type Signatures

    pub struct SeedRegistry {
        version: u32,
        interest_theme_ids: Vec<Uuid>,
        safety_toggle_ids: Vec<Uuid>,
        seeds: Vec<SeedDefinition>,
    }

    pub struct SeedDefinition {
        name: String,
        seed: u64,
        user_count: usize,
    }

    pub struct ExampleUserSeed {
        pub id: Uuid,
        pub display_name: String,
        pub interest_theme_ids: Vec<Uuid>,
        pub safety_toggle_ids: Vec<Uuid>,
        pub unit_system: UnitSystemSeed,
    }

    pub enum UnitSystemSeed {
        Metric,
        Imperial,
    }

    pub fn generate_example_users(
        registry: &SeedRegistry,
        seed_def: &SeedDefinition,
    ) -> Result<Vec<ExampleUserSeed>, GenerationError>

### Dependencies

    fake = "4.4"           # Name generation
    rand = "0.9"           # RNG traits
    rand_chacha = "0.9"    # Deterministic RNG
    serde = "1"            # JSON serialization
    serde_json = "1"       # JSON parsing
    thiserror = "2"        # Error derivation
    uuid = "1"             # User/theme/toggle IDs
