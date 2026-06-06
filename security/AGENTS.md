# Assistant instructions for `security/`

These instructions apply in addition to the repository root `AGENTS.md`.

## Scope

- Treat `security/` as JavaScript security automation for dependency audit
  policy, audit exception validation, and audit reporting.
- Keep security policy changes reflected in the relevant documentation under
  `docs/`, and update `docs/contents.md` if new long-lived security documents
  are added.
- Prefer root scripts and Makefile gates over ad hoc commands.

## TypeScript and JavaScript quality gates

- Validate formatting with `make check-fmt` from the repository root. Use
  `make fmt` to apply formatting fixes.
- Validate linting with `make lint` from the repository root.
- Validate repository JavaScript tests with `pnpm test` or `make test-frontend`
  from the repository root when security script tests are added or changed.
- Validate audit policy with `pnpm run audit:validate` from the repository root
  after changes to audit exceptions, audit schemas, or validator code.
- Run `make audit-node` when dependency audit behaviour or exception handling
  changes.

## Testing expectations

- Ensure new features are validated with unit tests and behavioural tests using
  `bun:test` where applicable, covering happy paths, unhappy paths, and relevant
  edge cases. The current root JavaScript test harness is Vitest; keep local
  tests compatible with the active repository harness unless the runner change
  is deliberate and documented.
- Add end-to-end tests where the change affects externally observable workflows,
  integration contracts, persistence, command-line behaviour, network
  boundaries, user interface flows, or other system-level behaviour.
- Use property tests with `fast-check` when audit validation or reporting
  introduces an invariant over a range of inputs, states, orderings, or
  transitions.
- For introduced axioms or contractual business logic, use an exhaustive proof,
  for example with LemmaScript. Proofs must be substantive, rigorous, and
  well-founded rather than restating the assumed property.
- Security tests must cover malformed inputs and policy bypass attempts where
  relevant, not only accepted configuration.
