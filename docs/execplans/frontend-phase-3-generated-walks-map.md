# Deliver generated walks and map experience

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

This phase proves Wildside's core product loop. A user can draft a walk, request
generation, follow progress, review the route on a stable map, inspect stops,
save the route, edit notes, and recover when location or Global Positioning
System (GPS) quality degrades.

## Constraints

Route generation must be durable and retry-safe. MapLibre ownership must remain
imperative and stable while React overlays change. WebSocket events may patch or
invalidate TanStack Query caches, but must not mutate view state directly.
Generated route plans must carry Point of Interest (POI) narrative
snippet lifecycle and cache metadata so offline rendering remains possible.

## Tolerances

Escalate if route-generation endpoints or events cannot express idempotency,
progress, sparse-data, cancellation, or conflict states. Escalate if map
fallbacks cannot meet accessibility requirements, or if geolocation permission
states require a product decision not captured in the design documents.

## Risks

The main risk is making map canvas lifecycle depend on React route renders.
Mitigate this by introducing a `MapStateProvider`, mocking MapLibre in tests,
and failing tests when overlay changes recreate the map instance.

## Plan

Follow `docs/frontend-roadmap.md` phase 3. Add route draft, request, status, and
route-plan schemas, then implement route-generation mutation, polling, and
WebSocket progress convergence. Build the wizard steps and transition wrapper so
impossible states cannot render.

Add lazy MapLibre loading, map state ownership, route-start and location
permission user experience (UX), map-led quick generation, Quick
Map tabs, Itinerary tabs, pedestrian instructions, degraded Global Positioning
System recovery, saved-route states, notes and progress hooks, route-plan
persistence, and Point of Interest narrative snippet lifecycle. Finish with
contract, state-transition, map-provider, and Playwright coverage.

## Validation

Run these commands from the repository root:

```bash
make fmt
make check-fmt
make lint
make test
make markdownlint
```

Browser validation must cover wizard-to-itinerary, map-led quick generation,
active navigation, degraded Global Positioning System, narrative snippet
fallback, saved routes, and share dialogs.

## Progress

- [x] Draft phase-level ExecPlan.
- Planned: Add route-generation schemas and async state.
- Planned: Deliver wizard and map-led generation surfaces.
- Planned: Keep MapLibre stable across overlays.
- Planned: Persist notes, progress, route plans, and narrative snippets.
- Planned: Verify generated-walk and map flows.

## Surprises & Discoveries

None yet.

## Decision Log

- 2026-04-28: Include map-led quick generation and degraded Global Positioning
  System recovery in the phase plan because both are now phase 3 roadmap tasks.

## Outcomes & Retrospective

Not started.
