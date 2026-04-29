# Evaluate deferred front-end extensions

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

This phase protects the core Progressive Web App (PWA) from
speculative scope. After phases 1-4 are stable, product and engineering can
decide which account, entitlement, pagination, native wrapper, notification,
community, audio, intent, feedback, and reporting features deserve their own
implementation slices.

## Constraints

Deferred work must not block phases 1-4 unless a product decision explicitly
promotes it. Visible write paths for community, feedback, reporting, or
entitlement must not ship without backend contracts, moderation expectations,
privacy expectations, and user-visible recovery copy.

## Tolerances

Escalate if a deferred feature becomes necessary for Minimum Viable Product
(MVP) acceptance, if entitlement rules affect access to the
core route-generation loop, or if native wrapper testing exposes service-worker,
storage, geolocation, or map behaviour that contradicts the Progressive Web App
implementation.

## Risks

The largest risk is mixing experiments into the core release. Mitigate this by
requiring each promoted extension to gain its own acceptance criteria,
contracts, validation plan, and roadmap placement before implementation.

## Plan

Follow `docs/frontend-roadmap.md` phase 5. Evaluate whether visible sign-in,
profile expansion, WebSocket display-name validation, entitlement, and free-tier
user experience (UX) belong in the first production release. Decide
which list surfaces need keyset pagination and only add pagination UI for
promoted surfaces.

After phase 4, reassess Capacitor, Tauri, push notifications, and background
sync as platform-specific extensions. Evaluate community ratings, reviews, route
sharing, audio guides, on-device intent recognition, feedback, and reporting as
separate product decisions with contracts and prototype success criteria.

## Validation

Run these commands for documentation-only decisions:

```bash
make fmt
make markdownlint
```

Any promoted implementation slice must define its own code, accessibility,
contract, and browser validation before work starts.

## Progress

- [x] Draft phase-level ExecPlan.
- Planned: Evaluate account, auth, entitlement, and free-tier user experience.
- Planned: Evaluate pagination requirements.
- Planned: Evaluate native wrappers and progressive platform features.
- Planned: Evaluate community, audio, intent, feedback, and reporting features.

## Surprises & Discoveries

None yet.

## Decision Log

- 2026-04-28: Treat phase 5 as product-decision work first, not a backlog of
  pre-approved implementation tasks.

## Outcomes & Retrospective

Not started.
