# Design plan: Backend sample data

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

No `PLANS.md` file exists in the repository root at the time of writing. If one
is added, this ExecPlan must be updated to follow it.

## Purpose / Big Picture

This document governs the **design activity** for the backend sample data
feature. The design itself lives at `docs/backend-sample-data-design.md`, and
this ExecPlan guides the work needed to author, review, and refine that design
and the corresponding roadmap updates.

Success is observable when:

- The design document exists at `docs/backend-sample-data-design.md` with a
  clear scope, decisions, and acceptance criteria.
- `docs/backend-roadmap.md` includes the planned implementation phases for the
  example-data crate and seeding feature.
- Documentation quality gates (`make fmt`, `make markdownlint`) pass.
- Review feedback can be incorporated without changing the ExecPlan's scope.

## Progress

- [x] (2026-01-03 17:41Z) Draft ExecPlan for backend sample data design.
- [x] (2026-01-03 18:40Z) Draft design document at
  `docs/backend-sample-data-design.md`.
- [x] (2026-01-03 18:40Z) Update `docs/backend-roadmap.md` with example-data
  implementation phases.
- [x] (2026-01-03 18:40Z) Incorporate named seed registry, CLI, and
  `ortho-config` requirements into the design document.
- [x] (2026-01-03 18:46Z) Capture code review feedback on the design document
  and roadmap updates.
- [x] (2026-01-03 18:46Z) Apply agreed design revisions and mark design work
  complete.

## Surprises & Discoveries

- Observation: The runtime server bootstrap does not currently create or attach
  a database pool, so any seeding logic must either add this wiring or remain
  gated on an existing pool. Evidence: `backend/src/main.rs` never calls
  `ServerConfig::with_db_pool`, and `backend/src/server/config.rs` marks it as
  reserved for future integration.

## Decision Log

- Decision: Split deliverables into a design doc at
  `docs/backend-sample-data-design.md` and a living ExecPlan at
  `docs/execplans/backend-sample-data-design.md`. Rationale: The design doc is
  expected to change in review, while the ExecPlan must persist to guide
  ongoing design work. Date/Author: 2026-01-03 / Codex

- Decision: Use a dedicated `example_data_runs` marker table to guarantee
  once-only seeding per seed key. Rationale: Explicit markers are more reliable
  than counting users and support safe concurrent startup behaviour.
  Date/Author: 2026-01-03 / Codex

- Decision: Keep the `example-data` crate independent of backend domain types.
  Rationale: Avoid circular dependencies and keep the generator reusable by
  adapters and tooling. Date/Author: 2026-01-03 / Codex

- Decision: Use deterministic RNG seeding by default with environment overrides
  for demo stability. Rationale: Stable demo data improves reproducibility
  across environments and tests. Date/Author: 2026-01-03 / Codex

- Decision: Store the seed registry in a JSON file and support multiple named
  seeds, with a CLI helper to add new seeds using `lexis` for naming.
  Rationale: JSON enables non-code updates and named seeds keep demos
  repeatable and discoverable. Date/Author: 2026-01-03 / Codex

- Decision: Use `ortho-config` to load configuration from settings files and
  environment overrides. Rationale: Configuration should be hierarchical and
  consistent with other backend settings. Date/Author: 2026-01-03 / Codex

## Outcomes & Retrospective

Design documentation is drafted, review feedback has been incorporated, and the
backend roadmap updated. No code changes are part of this design activity.

## Context and Orientation

The data model for user preferences and interests is defined in
`docs/wildside-pwa-data-model.md`, which specifies `UserPreferences` with
interest theme IDs, safety toggle IDs, and a unit system. The design needs to
align with those shapes while remaining backend-compatible.

Key files for this design activity:

- `docs/backend-sample-data-design.md`: the design document being authored.
- `docs/backend-roadmap.md`: the implementation roadmap to update.
- `docs/wildside-pwa-data-model.md`: data model requirements.
- `docs/wildside-backend-architecture.md`: architecture constraints.

## Plan of Work

1. Draft the design document in `docs/backend-sample-data-design.md`, covering
   scope, constraints, configuration, seeding workflow, and tests.
2. Update `docs/backend-roadmap.md` with an Example Data Seeding phase and the
   implementation tasks implied by the design.
3. Run documentation quality gates (`make fmt`, `make markdownlint`) to ensure
   formatting and lint compliance.
4. Capture review feedback from the design PR and update both the design doc
   and this ExecPlan to reflect any changes.

## Concrete Steps

Run these commands from the repository root after documentation updates:

1. Format Markdown:

   ```bash
   set -o pipefail
   timeout 300 make fmt 2>&1 | tee /tmp/wildside-fmt.log
   ```

2. Lint Markdown:

   ```bash
   set -o pipefail
   timeout 300 make markdownlint 2>&1 | tee /tmp/wildside-markdownlint.log
   ```

## Validation and Acceptance

Acceptance criteria:

- `docs/backend-sample-data-design.md` exists and captures the full design
  scope, decisions, and acceptance criteria for the example-data crate and
  seeding feature.
- `docs/backend-roadmap.md` includes an Example Data Seeding phase with clear
  tasks.
- `make fmt` and `make markdownlint` succeed.

## Idempotence and Recovery

Documentation edits and the formatting/lint commands are safe to re-run. If
review feedback changes the design scope, update this ExecPlan's `Decision Log`
and `Progress` sections to reflect the new direction.

## Artifacts and Notes

The design deliverables are:

- `docs/backend-sample-data-design.md`
- `docs/backend-roadmap.md`

Review feedback should be recorded in `Decision Log` and reflected in
`Progress`.

## Interfaces and Dependencies

The design document must specify:

- The public API surface of the `example-data` crate.
- The database marker table for once-only seeding.
- Seed registry format and CLI tooling for adding named seeds.
- Required configuration fields, settings file behaviour, and environment
  overrides.
- Startup wiring and logging behaviour.
- Testing strategy for deterministic generation and seeding idempotence.

## Revision note (required when editing an ExecPlan)

2026-01-03: Reframed the ExecPlan as the governing document for the design
activity and moved the actual design content to
`docs/backend-sample-data-design.md`.

2026-01-03: Added decisions for JSON seed registries, named seeds, CLI support,
and `ortho-config` configuration handling.

2026-01-03: Recorded review feedback integration and updated progress/outcomes.
