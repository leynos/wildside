# Assistant instructions for `packages/`

These instructions apply in addition to the repository root `AGENTS.md`.

## Scope

- Treat `packages/` as shared TypeScript workspace package code consumed by the
  PWA and repository tooling.
- Preserve package boundaries. Do not make a package depend on application
  internals unless a documented design decision explicitly permits it.
- Prefer workspace scripts and Makefile gates over ad hoc commands.

## TypeScript and JavaScript quality gates

- Validate formatting with `make check-fmt` from the repository root. Use
  `make fmt` to apply formatting fixes.
- Validate linting with `make lint` from the repository root.
- Validate type safety with `make typecheck` from the repository root. For this
  directory, the root target checks `packages/tokens/tsconfig.json` and
  `packages/types/tsconfig.json`.
- Validate tests with `make test-frontend` from the repository root. For focused
  package work, run the package script that exists locally, such as
  `pnpm --filter @app/tokens test` or `pnpm --filter @app/types build`.
- Run `pnpm --filter @app/tokens build` when token source files or token build
  utilities change. Run `pnpm --filter @app/types build` when shared type
  exports or schemas change.

## Testing expectations

- Ensure new features are validated with unit tests and behavioural tests using
  `bun:test` where applicable, covering happy paths, unhappy paths, and relevant
  edge cases. If the package already uses a different local test runner, keep the
  local convention and document any runner change before introducing it.
- Add end-to-end tests where the change affects externally observable workflows,
  integration contracts, persistence, command-line behaviour, network
  boundaries, user interface flows, or other system-level behaviour.
- Use property tests with `fast-check` when a change introduces an invariant over
  a range of inputs, states, orderings, or transitions.
- For introduced axioms or contractual business logic, use an exhaustive proof,
  for example with LemmaScript. Proofs must be substantive, rigorous, and
  well-founded rather than restating the assumed property.
- Shared package tests must protect the public package contract. Cover exported
  schemas, generated artefacts, token resolution, and package entry points when
  they change.
