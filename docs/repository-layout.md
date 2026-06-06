# Repository layout

This document orients contributors within the Wildside repository. It describes
the major paths and their responsibilities; it is not an exhaustive file list.

## Top-level tree

```plaintext
.
├── backend/
├── crates/
├── deploy/
├── docs/
├── frontend-pwa/
├── packages/
├── scripts/
├── security/
├── spec/
├── third_party/
└── tools/
```

_Figure 1: Simplified top-level repository tree._

## Path responsibilities

- `AGENTS.md` contains repository-wide assistant and contributor instructions.
- `backend/` contains Rust backend application code, migrations, fixtures, and
  backend tests.
- `crates/` contains shared Rust workspace crates that support backend and
  tooling features.
- `deploy/` contains container, Helm, nginx, and local deployment artefacts.
- `docs/` contains long-lived project documentation, design notes, runbooks,
  plans, diagrams, and documentation policy.
- `frontend-pwa/` contains the browser-facing Progressive Web Application
  (PWA), its scripts, source, and tests.
- `packages/` contains shared TypeScript workspace packages, including design
  tokens and cross-workspace types.
- `scripts/` contains repository automation and local development helper
  scripts.
- `security/` contains JavaScript audit validation, audit exception policy, and
  security reporting helpers.
- `spec/` contains API and protocol specifications used by generation,
  linting, and integration checks.
- `third_party/` contains vendored or externally sourced code kept under
  explicit repository control.
- `tools/` contains repository-specific developer and architecture tooling.

## Documentation paths

- `docs/contents.md` is the canonical documentation index.
- `docs/documentation-style-guide.md` is the source of truth for documentation
  style, naming, and standard document types.
- `docs/repository-layout.md` is the canonical repository layout guide.
- `docs/runbooks/` contains operational procedures.
- `docs/execplans/` contains execution plans for non-trivial implementation
  work.
- `docs/diagrams/` contains diagram assets referenced by documentation.
- `docs/investigations/` contains longer-lived investigation notes.

## Frontend and package paths

- `frontend-pwa/AGENTS.md` contains PWA-specific TypeScript and JavaScript
  guidance.
- `packages/AGENTS.md` contains guidance for shared TypeScript packages.
- `security/AGENTS.md` contains guidance for JavaScript audit and security
  automation.

Update this document when the repository structure changes enough that a new
contributor could otherwise follow outdated path guidance.
