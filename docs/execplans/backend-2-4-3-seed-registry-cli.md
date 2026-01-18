# Deliver seed registry CLI

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DONE

No `PLANS.md` file exists in the repository root at the time of writing. If
one is added, this ExecPlan must be updated to follow it.

## Purpose / Big Picture

Deliver a seed registry CLI that adds named seeds to the JSON registry using
`base-d` with the `eff_long` wordlist to generate memorable names, while
keeping the registry consistent and safe to update. After this change, a
developer can run a CLI command to append an entry to
`backend/fixtures/example-data/seeds.json` without editing JSON by hand, and
the update is validated and written atomically. Success is observable when the
CLI can add a seed, reject invalid inputs, and the full test suite passes with
both unit and behavioural coverage.

## Constraints

- The CLI must use the `example-data` crate for registry parsing and
  validation; it must not reimplement JSON parsing outside the crate.
- Direct stateful logic belongs behind ports; do not add new stateful logic to
  inbound HTTP adapters as part of this CLI work.
- The seed registry format and naming rules must remain compatible with
  `docs/backend-sample-data-design.md` and the existing registry file.
- The CLI must not depend on backend outbound adapters or Diesel modules.
- The CLI must not use `lexis`; use `base-d` with the `eff_long` wordlist
  instead.
- New Rust modules must begin with module-level `//!` comments.
- Avoid `println!`/`eprintln!` macros (Clippy forbids `print_stdout` and
  `print_stderr`); use `std::io` writes instead.
- All new public items require Rustdoc comments with examples where relevant.
- Keep any new file under 400 lines.
- Documentation must use en-GB-oxendict spelling and wrap paragraphs at 80
  columns.
- Update `docs/wildside-backend-architecture.md` with any design decisions
  taken while implementing this CLI.

## Tolerances (Exception Triggers)

- Scope: if implementation needs more than 12 files or more than 500 net lines
  of code, stop and escalate.
- Interface: if existing public APIs must change in a breaking way, stop and
  escalate.
- Dependencies: adding `base-d` is expected; adding any additional dependency
  beyond `base-d` requires escalation.
- Licensing: if `base-d` licence terms conflict with project policy, stop and
  escalate before proceeding.
- Iterations: if `make test` fails after three fix attempts, stop and
  escalate.
- Ambiguity: if CLI behaviour (defaults, name generation, or collision
  handling) has multiple valid interpretations, present options before
  proceeding.

## Risks

    - Risk: generated names are multi-word passphrases rather than adjective
      noun pairs, changing the expected naming semantics.
      Severity: low
      Likelihood: high
      Mitigation: document the new naming format and keep names hyphen-joined
      so they remain single tokens in the registry.

    - Risk: concurrent edits to the registry could be overwritten.
      Severity: medium
      Likelihood: low
      Mitigation: write atomically via a temp file in the same directory and
      consider a simple last-write-wins warning if the file changed between
      read and write.

    - Risk: generated names collide with existing seeds.
      Severity: low
      Likelihood: medium
      Mitigation: detect collisions and retry name generation with a new seed
      value up to a bounded limit.

    - Risk: tests mutate the real fixture file.
      Severity: medium
      Likelihood: low
      Mitigation: copy the registry into a per-test temp directory and operate
      on the copy only.

## Progress

    - [x] (2026-01-18 00:00Z) Draft ExecPlan for seed registry CLI.
    - [x] (2026-01-18) Confirm `base-d` licence and API usage for name
      generation.
    - [x] (2026-01-18) Add registry update API and tests in `example-data`
      crate.
    - [x] (2026-01-18) Implement CLI binary and behavioural tests.
    - [x] (2026-01-18) Update architecture doc and mark roadmap task 2.4.4 as
      done.
    - [x] (2026-01-18) Run `make check-fmt`, `make lint`, and `make test`.

## Surprises & Discoveries

    - Observation: `rstest-bdd` step placeholders include surrounding quotes
      when the feature text includes quoted strings.
      Evidence: Duplicate-name scenario passed quoted values through to the
      JSON seed registry and caused parse errors.
      Impact: Step patterns now include explicit quotes to capture the inner
      value only.

## Decision Log

    - Decision: Implement the CLI as a binary target in the `example-data`
      crate (`example-data-seed`) so it can reuse registry types directly.
      Rationale: Keeps seed registry logic close to the data model and avoids
      backend dependencies.
      Date/Author: 2026-01-18 / Plan author.

    - Decision: Write registry updates atomically with a temp file and rename.
      Rationale: Prevents partial writes if the CLI crashes mid-write.
      Date/Author: 2026-01-18 / Plan author.

    - Decision: Generate seed names using `base-d` with the `eff_long`
      dictionary and join words with hyphens.
      Rationale: Avoids GPL-licensed dependencies while keeping names readable
      and registry-friendly as single tokens.
      Date/Author: 2026-01-18 / Plan author.

## Outcomes & Retrospective

- Delivered the seed registry CLI with `base-d` `eff_long` naming and atomic
  writes, plus unit and behavioural tests for the update flow.
- Documentation now records the naming change and roadmap item 2.4.4 is
  marked done.
- Quality gates (`make check-fmt`, `make lint`, `make test`) completed
  successfully.

## Context and Orientation

The seed registry lives at `backend/fixtures/example-data/seeds.json` and is
parsed by the `example-data` crate in `crates/example-data/src/registry.rs`.
The registry format uses camelCase fields: `version`, `interestThemeIds`,
`safetyToggleIds`, and `seeds` with `name`, `seed`, and `userCount` entries.

Relevant code locations:

- `crates/example-data/src/registry.rs` for registry parsing and validation.
- `crates/example-data/src/error.rs` for registry error enums.
- `crates/example-data/tests/` for unit and behavioural test patterns.
- `backend/fixtures/example-data/seeds.json` for the default registry file.
- `docs/backend-sample-data-design.md` for functional expectations.
- `docs/wildside-backend-architecture.md` for architecture decisions.
- `docs/backend-roadmap.md` for updating task 2.4.4 status.

`rstest` is used for unit tests, and `rstest-bdd` v0.3.2 is used for
behavioural tests. Where Postgres is required, tests must rely on the
`pg_embedded_setup_unpriv` fixtures and guidance in
`docs/pg-embed-setup-unpriv-users-guide.md`. This CLI does not touch Postgres
by design, so no new database fixtures are expected.

## Plan of Work

Stage A: Confirm inputs and dependency constraints.

- Read `docs/backend-sample-data-design.md` to confirm CLI expectations.
- Verify `base-d` API and licence notes, then decide how to handle any
  licensing constraints.
- Confirm the registry file path and default values for `userCount` and `seed`.

Stage B: Add registry update support in `example-data`.

- Extend `SeedDefinition` with a constructor for CLI usage.
- Add `SeedRegistry` helpers to append a seed, validate uniqueness, and render
  JSON for writing.
- Add a safe write helper that writes to a temp file then renames it into
  place.
- Add unit tests (rstest) for new registry update behaviours, including
  duplicate name detection and JSON output stability.

Stage C: Implement the CLI binary.

- Add a binary target `example-data-seed` under
  `crates/example-data/src/bin/`.
- Parse CLI arguments (`--registry`, `--name`, `--seed`, `--user-count`) using
  a minimal manual parser to avoid new dependencies.
- If `--name` is absent, use `base-d` with the `eff_long` wordlist to encode
  the seed bytes and join the resulting words with hyphens.
- Ensure the CLI reads the registry via `SeedRegistry::from_file`, appends the
  new seed via the registry helper, and writes atomically.
- Emit a concise success message to stdout via `std::io`.

Stage D: Behavioural tests and documentation updates.

- Add BDD scenarios under `crates/example-data/tests/features/` covering:
  - happy path seed creation with generated name;
  - explicit name override;
  - duplicate name rejection;
  - invalid registry JSON error handling.
- Add a behavioural test driver in `crates/example-data/tests/` using
  `rstest-bdd` v0.3.2, following existing `ScenarioState` patterns.
- Update `docs/wildside-backend-architecture.md` with a design decision note
  about the seed registry CLI and atomic update approach.
- Mark roadmap item 2.4.4 as done in `docs/backend-roadmap.md`.

Each stage ends with running the relevant tests and updating `Progress`.
Do not proceed if any stage validation fails.

## Concrete Steps

Run these from the repository root unless noted otherwise.

1. Confirm current branch and gather context:

    git branch --show
    rg -n "seed registry|example-data-seed" docs/backend-sample-data-design.md
    rg -n "SeedRegistry" crates/example-data/src

2. Implement Stage B changes and unit tests, then run the unit test subset:

    set -o pipefail
    timeout 300 cargo test -p example-data registry 2>&1 | \
      tee /tmp/test-example-data-registry-$(git branch --show).out

3. Implement Stage C CLI binary and behaviour tests, then run example-data
   tests:

    set -o pipefail
    timeout 300 cargo test -p example-data 2>&1 | \
      tee /tmp/test-example-data-$(git branch --show).out

4. Run full quality gates once all changes are in place:

    set -o pipefail
    timeout 300 make check-fmt 2>&1 | \
      tee /tmp/check-fmt-$(git branch --show).out

    set -o pipefail
    timeout 300 make lint 2>&1 | \
      tee /tmp/lint-$(git branch --show).out

    set -o pipefail
    timeout 300 make test 2>&1 | \
      tee /tmp/test-$(git branch --show).out

If tests fail due to Postgres bootstrap, follow
`docs/pg-embed-setup-unpriv-users-guide.md` and re-run with the appropriate
fixture. Use `SKIP_TEST_CLUSTER=1` only for local triage; do not commit with
skipped integration coverage.

## Validation and Acceptance

Acceptance criteria:

- Running `cargo run -p example-data --bin example-data-seed -- \
  --registry backend/fixtures/example-data/seeds.json` appends a new seed and
  prints a success line that includes the hyphen-joined seed name.
- The CLI rejects a duplicate seed name with a clear error message.
- Unit tests cover registry update helpers using `rstest`.
- Behavioural tests cover CLI scenarios using `rstest-bdd` v0.3.2.
- `make check-fmt`, `make lint`, and `make test` all succeed.
- `docs/wildside-backend-architecture.md` records the CLI decision, and
  `docs/backend-roadmap.md` marks task 2.4.4 as done.

Quality criteria (what "done" means):

- Tests: `make test` passes; the new BDD feature runs and passes.
- Lint/typecheck: `make lint` passes without warnings.
- Formatting: `make check-fmt` passes.

## Idempotence and Recovery

CLI writes are safe to re-run because each new seed name is unique; if a
collision occurs, the CLI must surface the error without overwriting existing
entries. The atomic write strategy ensures partial writes do not corrupt the
registry. If a write fails, remove any temp file left in the registry
directory and re-run the CLI.

## Artifacts and Notes

Expected CLI output example (stderr on failure; stdout on success):

    Added seed "brisk-lantern-pond" (seed=2026, userCount=12) to
    backend/fixtures/example-data/seeds.json

Keep the output short and avoid printing the full registry.

## Interfaces and Dependencies

Planned additions (exact signatures to confirm during implementation):

- `crates/example-data/src/registry.rs`
  - Add `SeedDefinition::new(name: String, seed: u64, user_count: usize) ->
    Self`.
  - Add `SeedRegistry::append_seed(&self, seed: SeedDefinition) ->
    Result<Self, RegistryError>` to return a new registry with the added seed
    and validate unique names.
  - Add `SeedRegistry::to_json_pretty(&self) -> Result<String, RegistryError>`
    or an equivalent helper for deterministic JSON output.

- `crates/example-data/src/error.rs`
  - Add `RegistryError::DuplicateSeedName { name: String }` (or a dedicated
    update error enum if needed) for collision reporting.

- `crates/example-data/src/bin/example_data_seed.rs`
  - CLI entry point with manual argument parsing.
  - Use `base-d` with `wordlists::eff_long()` to derive names from the seed
    bytes when `--name` is absent.
  - Use `rand` to generate a default seed when one is not provided.

Dependencies:

- Add `base-d = "<approved version>"` to
  `crates/example-data/Cargo.toml`.
- No other new dependencies are planned.

## Revision note (required when editing an ExecPlan)

2026-01-18: Initial draft for task 2.4.4 seed registry CLI.
2026-01-18: Updated dependency plan to use `base-d` `eff_long` wordlist and
hyphen-joined passphrase names.
2026-01-18: Recorded progress updates and noted `rstest-bdd` quote handling.
2026-01-18: Marked plan complete with outcomes and gating results.
