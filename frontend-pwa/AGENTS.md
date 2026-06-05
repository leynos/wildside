# Assistant instructions for `frontend-pwa/`

These instructions apply in addition to the repository root `AGENTS.md`.

## Scope

- Treat `frontend-pwa/` as the browser-facing Progressive Web Application
  (PWA) workspace.
- Keep work aligned with the frontend architecture and testing references in
  `docs/contents.md`, especially the Wildside PWA design, data model, sitemap,
  accessibility, localization, and front-end stack documents.
- Prefer workspace scripts and Makefile gates over ad hoc commands.

## TypeScript and JavaScript quality gates

- Validate formatting with `make check-fmt` from the repository root. Use
  `make fmt` to apply formatting fixes.
- Validate linting with `make lint` from the repository root.
- Validate type safety with `make typecheck` from the repository root. For this
  workspace, the root target runs `tsc --noEmit -p frontend-pwa/tsconfig.json`.
- Validate tests with `make test-frontend` from the repository root. For focused
  PWA work, `pnpm --filter frontend-pwa test` runs the local Vitest suite.
- Run `pnpm --filter frontend-pwa build` when a change affects routing, bundling,
  public assets, generated API clients, or production-only behaviour.

## Testing expectations

- Ensure new features are validated with unit tests and behavioural tests using
  the existing Vitest setup where applicable. Cover happy paths, unhappy paths,
  and relevant edge cases.
- Keep behavioural tests under `frontend-pwa/tests/` using the existing
  `*.behaviour.test.ts` naming pattern. Keep narrow unit tests near the relevant
  test area using `*.unit.test.ts` when that is already the local convention.
- Add end-to-end tests where the change affects externally observable workflows,
  integration contracts, persistence, command-line behaviour, network
  boundaries, user interface flows, or other system-level behaviour.
- Use property tests with `fast-check` when a change introduces an invariant over
  a range of inputs, states, orderings, or transitions.
- For introduced axioms or contractual business logic, use an exhaustive proof,
  for example with LemmaScript. Proofs must be substantive, rigorous, and
  well-founded rather than restating the assumed property.
- Keep tests deterministic. Inject or isolate time, random number generation,
  storage, fetch, environment variables, and other non-deterministic boundaries.
