# Deliver offline, safety, and completion trust

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

This phase completes the PWA reliability promise. Users can install the app,
load the app shell offline, manage offline bundles, persist safety preferences,
complete a walk, and see completion summaries that emphasize discovered places
rather than fitness scoring.

## Constraints

Service-worker cache policy must be explicit and tested. Tile bytes must live
outside React state and TanStack Query. Offline writes must use the same outbox
and idempotency strategy as earlier phases. Safety preferences must store
semantic descriptor IDs rather than UI labels or CSS classes.

## Tolerances

Escalate if browser quota behaviour prevents reliable bundle status updates, if
service-worker update UX requires an unplanned product decision, or if walk
completion contracts cannot avoid duplicate sessions after offline retry.

## Risks

Offline bundle work can blur cache ownership. Mitigate this by separating
bundle manifests, tile Cache Storage, route-plan persistence, and outbox writes
in tests and documentation.

## Plan

Follow `docs/frontend-roadmap.md` phase 4. Add the Web App Manifest,
installability metadata, service worker, app-shell precache, navigation
fallback, runtime cache policies, update states, and network-status states.

Implement offline bundle manifest queries, dashboard states, add-area dialog,
manage mode, delete undo, storage pressure handling, tile prefetch, retry, and
eviction. Add safety descriptor registries, preference accordions, presets,
saved dialog, and conflict recovery. Finish with walk-session creation,
completion mutations, completion summary, sharing, save/remix transitions, and
offline/safety/completion tests.

## Validation

Run these commands from the repository root:

```bash
make fmt
make check-fmt
make lint
make test
make markdownlint
```

Preview-build validation must show Lighthouse PWA installability, offline
app-shell load, tested cache policy, durable bundle manifest state, safety
conflict recovery, and accessible completion in at least one non-default locale.

## Progress

- [ ] Draft phase-level ExecPlan.
- [ ] Add manifest, service worker, and cache policies.
- [ ] Deliver offline bundle lifecycle.
- [ ] Persist safety preferences with conflict recovery.
- [ ] Record walks and render completion summaries.
- [ ] Verify offline, safety, and completion reliability.

## Surprises & Discoveries

None yet.

## Decision Log

- 2026-04-28: Keep tile bytes explicitly outside React and TanStack Query in
  the plan because that ownership boundary is central to the phase.

## Outcomes & Retrospective

Not started.
