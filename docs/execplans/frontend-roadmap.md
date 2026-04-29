# Track front-end roadmap delivery

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

This plan tracks delivery of the Wildside front-end roadmap from source
reconciliation through deferred product decisions. It gives a contributor one
place to see the execution order, the governing documents, the validation gates,
and the phase-level plans that should be updated as implementation progresses.

The observable outcome is a front-end roadmap that can be implemented without
hunting through the pull request history. A contributor can start from this
file, open the phase-specific ExecPlan, follow the cited roadmap tasks, and run
the documented Makefile gates before committing.

## Constraints

`docs/frontend-roadmap.md` remains the implementation task catalogue. This plan
tracks execution and links to phase plans; it must not accumulate product
requirements that belong in design documents, architecture documents, API
schemas, or ADRs.

`frontend-pwa/package.json` remains the source of truth for installed
front-end dependencies. `docs/v2a-front-end-stack.md` documents the current
package state and target-stack boundary. Any change that adds target-stack
dependencies must update package files, lockfiles, developer guidance, and the
relevant phase plan in the same change.

Documentation changes must use en-GB Oxford spelling conventions used by this
repository, including `-ize` forms such as localization and customization.

## Tolerances

Escalate before proceeding if a roadmap task requires a product decision not
captured in a design document, an API contract change outside the front-end
scope, a new dependency that contradicts `docs/v2a-front-end-stack.md`, or a
quality gate that cannot run in local development and CI.

Escalate if implementation would make the roadmap the primary source of a
schema shape, policy decision, accessibility requirement, cache rule, or
entitlement rule.

## Risks

The main risk is treating the roadmap as a design document. Mitigate this by
moving substantive decisions to `docs/wildside-pwa-design.md`,
`docs/wildside-pwa-data-model.md`, API specifications, or ADRs, then updating
the roadmap with citations.

The second risk is dependency drift between the target v2a stack and the
checked-in package. Mitigate this by verifying `frontend-pwa/package.json` and
`bun.lock` whenever a phase adds or removes tooling.

The third risk is inaccessible UI patterns spreading before the test and lint
gates exist. Mitigate this by completing phase 1 accessibility, semantic CSS,
and Playwright gates before feature slices become broad.

## Plan

Start with phase 0 in `docs/execplans/frontend-phase-0-source-reconciliation.md`.
Resolve source authority, reconcile design documents, import v2a lint policy,
and import the token and design-system source into the local token pipeline.

Proceed to phase 1 in `docs/execplans/frontend-phase-1-build-spine.md`. Ratify
the stack boundary, establish application shell and route metadata, add
schema-validated API boundaries, and make front-end accessibility and semantic
quality gates executable.

Proceed to phase 2 in
`docs/execplans/frontend-phase-2-catalogue-onboarding-discovery.md`. Deliver the
catalogue-led onboarding and discovery flow from localized entity data,
preference hooks, and stale or offline catalogue states.

Proceed to phase 3 in `docs/execplans/frontend-phase-3-generated-walks-map.md`.
Deliver route generation, wizard flow, map-led quick generation, MapLibre
stability, active navigation, degraded Global Positioning System (GPS) recovery,
saved routes, notes, progress, and Point of Interest (POI) narrative snippet
lifecycle.

Proceed to phase 4 in
`docs/execplans/frontend-phase-4-offline-safety-completion.md`. Deliver
installability, service-worker cache policy, offline bundles, safety
preferences, conflict recovery, walk completion, and completion summaries.

Use phase 5 in `docs/execplans/frontend-phase-5-deferred-extensions.md` to
evaluate account, entitlement, pagination, native wrappers, notifications,
community features, audio, intent, feedback, and reporting only after the core
Progressive Web Application (PWA) is trustworthy.

## Validation

For documentation-only roadmap and plan updates, run:

```bash
make fmt
make markdownlint
```

For changes touching Mermaid diagrams, also run:

```bash
make nixie
```

For code changes under `frontend-pwa/` or `packages/tokens/`, run the full
repository gates:

```bash
make check-fmt
make lint
make test
```

The expected result is that each command exits with status `0`. Capture output
with `tee` using the repository command guidance when running gates before a
commit.

## Progress

- [x] 2026-04-29: Create branch-level front-end roadmap ExecPlan.
- [ ] Complete phase 0 source reconciliation.
- [ ] Complete phase 1 build spine.
- [ ] Complete phase 2 catalogue onboarding and discovery.
- [ ] Complete phase 3 generated walks and map experience.
- [ ] Complete phase 4 offline, safety, and completion trust.
- [ ] Complete phase 5 deferred extension decisions.

## Surprises & Discoveries

- 2026-04-29: The repository already had phase-specific front-end ExecPlans, but
  it lacked the branch-level plan named after the `frontend-roadmap` branch.

## Decision Log

- 2026-04-29: Keep this document as the roadmap progress tracker and keep
  implementation details in the phase-specific ExecPlans. This satisfies the
  need for one canonical execution plan without duplicating the task catalogue.

## Outcomes & Retrospective

Not started.
