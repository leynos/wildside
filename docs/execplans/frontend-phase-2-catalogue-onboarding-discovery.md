# Deliver catalogue onboarding and discovery

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

This phase delivers the first complete user journey: launch Wildside, choose
interests, browse the catalogue, filter discovery content, customize route
preferences, and preserve choices locally. After completion, visible catalogue
surfaces render from localized entity projections rather than hard-coded card
copy.

## Constraints

Use the entity and descriptor model from
`docs/data-model-driven-card-architecture.md` and
`docs/wildside-pwa-data-model.md`. Entity names, descriptions, badges, and card
copy belong to localized entity data, while UI chrome belongs to the
translation layer. Discovery and preference writes must use the local-first
query and outbox boundaries from phase 1.

## Tolerances

Escalate if backend catalogue or preference contracts are missing required
fields, if fixture fallback would require component-specific data shapes, or if
guest and authenticated preference behaviour conflicts with the API contract.

## Risks

Catalogue work can drift into bespoke UI data models. Mitigate this by adding
shared entity types and descriptor registries before rendering screens, and by
testing fallback locale ordering and International System of Units (SI)-unit
formatting before adding more card surfaces.

## Plan

Follow `docs/frontend-roadmap.md` phase 2. Add shared entity, localization,
media, and descriptor types, then implement deterministic locale resolution for
entity data and UI chrome. Reshape mockup catalogue fixtures into
backend-compatible projections and add query adapters with fixture fallback and
stale catalogue states.

Implement Welcome, Discover, Explore, Customize, and bottom navigation as one
accessible route journey. Add preference and interest hooks with optimistic
updates, guest/authenticated resolution, and demo-data documentation. Finish
with component, hook, accessibility, and Playwright coverage for onboarding,
stale catalogue, and offline fallback states.

## Validation

Run these commands from the repository root:

```bash
make fmt
make check-fmt
make lint
make test
make markdownlint
```

Success means the onboarding and discovery flow works with keyboard navigation,
accessible names, localized entity data, stale catalogue copy, and offline
fallback behaviour.

## Progress

- [x] Draft phase-level ExecPlan.
- [ ] Add shared entity and descriptor vocabulary.
- [ ] Implement catalogue-backed Welcome, Discover, Explore, and Customize.
- [ ] Persist interests and preferences.
- [ ] Verify the catalogue journey.

## Surprises & Discoveries

None yet.

## Decision Log

- 2026-04-28: Keep this plan focused on the first user-visible catalogue slice;
  route generation remains phase 3.

## Outcomes & Retrospective

Not started.
