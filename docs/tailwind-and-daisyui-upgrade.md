# Tailwind and DaisyUI upgrade (frontend-pwa)

Last updated: 14 December 2025

## Purpose

The Wildside mockup architecture is designed for Tailwind v4 and DaisyUI v5.
The current `frontend-pwa/` workspace uses Tailwind v3 and DaisyUI v4.

This document records the upgrade work item so the PWA design can reference a
tracked migration plan with clear scope boundaries and acceptance criteria.

## Scope

In scope:

- Upgrade `frontend-pwa/` to Tailwind v4 and DaisyUI v5.
- Align the design token pipeline with Tailwind v4 conventions.
- Keep styling semantics stable (Radix state attributes, DaisyUI roles, token
  names) while the underlying versioned plumbing changes.

Out of scope:

- UI redesign work.
- Route-level feature implementation beyond what is required to validate the
  upgrade.
- Any backend work.

## Current state (repository)

- Tailwind: v3 (declared in `frontend-pwa/package.json`).
- DaisyUI: v4 (declared in `frontend-pwa/package.json`).

## Target state

- Tailwind: v4.
- DaisyUI: v5.

## Work plan

1. Update `frontend-pwa/package.json` dependency versions.
2. Update Tailwind configuration and any build tooling required by v4 (for
   example `@theme` integration where applicable).
3. Update DaisyUI configuration to v5 conventions and verify theme variable
   mapping still matches token roles.
4. Verify token generation and consumption paths still work end-to-end:
   `packages/tokens/` → PWA CSS variables → Tailwind utilities → DaisyUI roles.
5. Validate rendering and accessibility baselines against the mockup
   expectations:
   - Focus rings and interactive states remain visible.
   - Colour roles still meet contrast requirements.
   - Component behaviour remains driven by semantics and Radix state.

## Acceptance criteria

- `make fmt` is clean.
- `make lint` is clean.
- `make test` is clean.
- `make markdownlint` is clean.
- `make nixie` is clean (Mermaid rendering remains valid).
- The PWA dev build (`frontend-pwa/`) starts and renders the app shell without
  missing styles.

## References

- `wildside-pwa-design.md` (version alignment section).
