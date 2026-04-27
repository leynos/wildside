# Documentation index

## Project architecture

- [Wildside high-level design](wildside-high-level-design.md) – strategic
  blueprint and product vision. *Audience: stakeholders and all contributors.*
- [Repository design guide](repository-structure.md) – explains repository
  layout and request flow. *Audience: new contributors.*
- [Wildside backend: functional design specification](wildside-backend-design.md)
  – outlines backend components and tasks. *Audience: backend developers.*
- [Wildside backend architecture](wildside-backend-architecture.md) – hexagonal
  modular monolith overview and domain boundaries. *Audience: backend
  developers.*
- [Backend MVP architecture and observability](backend-design.md) – details
  monolithic backend and observability plan. *Audience: backend developers.*
- [Values class diagram](values-class-diagram.mmd) – Mermaid diagram of Helm
  chart values. *Audience: platform engineers.*

## Architecture decision records

- [Architecture Decision Record (ADR) 001: WebSockets on
  actix-ws](adr-001-websockets-on-actix-ws.md) – rationale for migrating the
  WebSocket adapter to `actix-ws`. *Audience: backend developers.*

## Frontend development

- [Wildside front-end roadmap](frontend-roadmap.md) – GIST-aligned
  implementation roadmap for the PWA front-end. *Audience: frontend developers
  and project planners.*
- [Pure, accessible, and localizable React components](pure-accessible-and-localizable-react-components.md)
  – building accessible, localizable components with Radix, TanStack, and
  DaisyUI. *Audience: frontend developers.*
- [Wildside PWA design](wildside-pwa-design.md) – frontend architecture,
  offline-first behaviour, and localization strategy. *Audience: frontend
  developers and contributors.*
- [Wildside PWA data model](wildside-pwa-data-model.md) – mockup-derived,
  backend-compatible entity shapes for the PWA. *Audience: frontend and backend
  developers.*
- [Wildside UX state graph](wildside-ux-state-graph-v0.1.json) –
  machine-readable map of routes, states, transitions, API contracts, and test
  recommendations. *Audience: frontend developers and QA engineers.*
- [Wildside PWA sitemap](sitemap.md) – route structure, navigation groups, and
  user flows for the PWA. *Audience: frontend developers and UX reviewers.*
- [v2a front-end stack](v2a-front-end-stack.md) – Bun, Vite, React, TanStack,
  Tailwind, DaisyUI, Radix, i18n, map, and testing stack reference. *Audience:
  frontend developers.*
- [Data model-driven card architecture](data-model-driven-card-architecture.md)
  – entity-owned localization and SI-unit card modelling. *Audience: frontend
  developers and data model maintainers.*
- [Architecting resilient local-first applications in React](local-first-react.md)
  – strategies for offline-first apps using Zustand and TanStack Query.
  *Audience: frontend developers.*
- [High-velocity, accessibility-first component testing](high-velocity-accessibility-first-component-testing.md)
  – Vitest and Playwright strategy for accessible components. *Audience:
  frontend developers and QA engineers.*
- [Building accessible and responsive progressive web applications](building-accessible-and-responsive-progressive-web-applications.md)
  – standards-focused guide to PWA installability, service workers, responsive
  design, and WCAG. *Audience: frontend developers and QA engineers.*
- [React and Tailwind with Bun](react-tailwind-with-bun.md) – Bun-centric
  React, Tailwind, Vite, and static preview reference. *Audience: frontend
  developers.*
- [Tailwind and DaisyUI upgrade](tailwind-and-daisyui-upgrade.md) – tracked
  work item for aligning the PWA workspace with Tailwind v4 and DaisyUI v5.
  *Audience: frontend developers and contributors.*
- [Tailwind v3 to v4 migration guide](tailwind-v3-v4-migration-guide.md) –
  migration notes for Tailwind CSS v4’s CSS-first configuration and breaking
  changes. *Audience: frontend developers.*
- [Tailwind CSS v4 guide](tailwind-v4-guide.md) – Tailwind v4 setup, custom
  utilities, variants, theming, and utility reference. *Audience: frontend
  developers.*
- [daisyUI v5 guide](daisyui-v5-guide.md) – DaisyUI v5 installation,
  configuration, theme roles, and component class reference. *Audience:
  frontend developers.*
- [Semantic Tailwind with daisyUI best practice](semantic-tailwind-with-daisyui-best-practice.md)
  – semantic HTML, Radix state styling, Tailwind utilities, DaisyUI roles, and
  token guidance. *Audience: frontend developers.*
- [Enforcing semantic Tailwind best practice](enforcing-semantic-tailwind-best-practice.md)
  – Biome, GritQL, Semgrep, and Stylelint rules for semantic, token-driven
  markup. *Audience: frontend developers and tooling maintainers.*

## Rust testing practices

- [Reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md)
  – using the `mockable` crate for deterministic tests. *Audience: Rust
  developers.*
- [Guide to ergonomic and DRY Rust doctests](rust-doctest-dry-guide.md) –
  patterns for maintainable doctests. *Audience: Rust developers.*
- [Mastering test fixtures in Rust with `rstest`](rust-testing-with-rstest-fixtures.md)
  – fixture and parameterized testing techniques. *Audience: Rust developers.*

## Infrastructure and delivery

- Infrastructure automation, GitOps workflows, and ephemeral preview
  environments are documented in the Nile Valley repository
  (`../../nile-valley`). This repository keeps the application code, container
  images, and Helm chart that Nile Valley deploys.

## Operational runbooks

- [OSM ingestion end-to-end runbook](runbooks/osm-ingestion-e2e.md) – operator
  procedure for executing and verifying `ingest-osm` runs, including
  deterministic reruns. *Audience: backend operators and developers.*
- [Session signing key rotation](runbooks/session-key-rotation.md) – procedure
  for rotating backend session signing keys in Kubernetes. *Audience: platform
  engineers and operators.*

## Developer guidelines and tooling

- [Documentation style guide](documentation-style-guide.md) – conventions for
  clear, consistent docs. *Audience: all contributors.*
- [Scripting standards](scripting-standards.md) – Python-first automation
  guidance covering `uv`, `plumbum`, and testing expectations. *Audience:
  automation authors.*
- [Complexity antipatterns and refactoring strategies](complexity-antipatterns-and-refactoring-strategies.md)
  – managing code complexity. *Audience: implementers and maintainers.*
- [A command-line wizard’s guide to srgn](srgn.md) – using `srgn` for
  syntactic code refactoring. *Audience: developers performing code changes.*
- [Biome configuration schema](biome-schema.json) – JSON schema for
  `biome.json`. *Audience: contributors editing Biome settings.*
