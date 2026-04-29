# Establish the front-end build spine

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

This phase turns `frontend-pwa/` from a minimal shell into the stable
application spine required by later feature slices. After completion, a
developer can run the app, navigate the route shell, rely on validated API
boundaries, persist offline writes, and run accessibility-first quality gates.

## Constraints

Do not add target-stack dependencies until the stack alignment task accepts
them and updates `frontend-pwa/package.json`, the lockfile, and developer
documentation. Generated API clients, token outputs, and route metadata must be
owned by repeatable scripts. Server state belongs in TanStack Query, durable
offline writes belong behind the outbox boundary, and feature views must not
mutate shared caches directly.

## Tolerances

Escalate if the chosen stack conflicts with `docs/v2a-front-end-stack.md`, if
route-state requirements cannot be represented without a new architectural
decision, or if new test gates need browser or network capabilities unavailable
in CI.

## Risks

The largest risk is broad dependency churn before architectural boundaries are
ready. Mitigate this by landing stack alignment, route shell, schema boundary,
and test harness changes as separate commits with focused verification.

## Plan

Follow `docs/frontend-roadmap.md` phase 1. First, record the stack alignment
decision and normalize package versions, scripts, and token generation. Next,
replace the single app view with a feature-first shell, implement route
metadata, and map user experience (UX) graph states to routes or documented
deferrals.

Then generate and wrap the OpenAPI REST client, introduce query-key factories,
add Dexie-backed outbox and offline bundle manifest storage, and define the
WebSocket event boundary. Finish by making component, accessibility,
Playwright, semantic CSS, and documentation gates executable from Makefile
targets.

## Validation

Run these commands from the repository root:

```bash
make fmt
make check-fmt
make lint
make test
make markdownlint
```

If Mermaid diagrams or routing diagrams change, also run:

```bash
make nixie
```

Success means the front-end shell builds, tests run through Makefile targets,
route metadata is covered by tests, API responses are schema-validated, and the
developer guide documents the workflow.

## Progress

- [x] Draft phase-level ExecPlan.
- [ ] Ratify stack and script boundaries.
- [ ] Establish shell, routes, providers, and metadata.
- [ ] Add validated API, query, outbox, and WebSocket boundaries.
- [ ] Add accessibility, semantic CSS, and documentation gates.

## Surprises & Discoveries

None yet.

## Decision Log

- 2026-04-28: Keep implementation order aligned with roadmap sections
  1.1-1.4 because later feature slices depend on these boundaries.

## Outcomes & Retrospective

Not started.
