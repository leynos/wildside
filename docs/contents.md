# Documentation contents

- [Documentation contents](contents.md) – the canonical index for project
  documentation. _Audience: all readers._

## Project architecture

- [Wildside high-level design](wildside-high-level-design.md) – strategic
  blueprint and product vision. _Audience: stakeholders and all contributors._
- [Repository design guide](repository-structure.md) – explains repository
  layout and request flow. _Audience: new contributors._
- [Repository layout](repository-layout.md) – maps the major repository paths
  and their responsibilities. _Audience: new contributors and maintainers._
- [Wildside backend: functional design specification](wildside-backend-design.md)
  – outlines backend components and tasks. _Audience: backend developers._
- [Wildside backend architecture](wildside-backend-architecture.md) – hexagonal
  modular monolith overview and domain boundaries. _Audience: backend
  developers._
- [Backend MVP architecture and observability](backend-design.md) – details
  monolithic backend and observability plan. _Audience: backend developers._
- [Local k3d preview and Nile Valley integration design](local-k8s-preview-design.md)
  – describes the backend health, container, Helm, and local preview contracts.
  _Audience: backend developers and platform engineers._
- [Values class diagram](values-class-diagram.mmd) – Mermaid diagram of Helm
  chart values. _Audience: platform engineers._

## Architecture decision records

- [Architecture Decision Record (ADR) 001: WebSockets on actix-ws](adr-001-websockets-on-actix-ws.md)
  – rationale for migrating the WebSocket adapter to `actix-ws`. _Audience:
  backend developers._

## Frontend development

- [Wildside front-end roadmap](frontend-roadmap.md) – GIST-aligned
  implementation roadmap for the Progressive Web Application (PWA) front-end.
  _Audience: frontend developers and project planners._
- [Front-end source authority catalogue](frontend-source-authority-catalogue.md)
  – ownership map for front-end requirements and reconciliation follow-ups.
  _Audience: frontend developers, reviewers, and project planners._
- [Pure, accessible, and localizable React components](pure-accessible-and-localizable-react-components.md)
  – building accessible, localizable components with Radix, TanStack, and
  DaisyUI. _Audience: frontend developers._
- [Wildside PWA design](wildside-pwa-design.md) – frontend architecture,
  offline-first behaviour, and localization strategy. _Audience: frontend
  developers and contributors._
- [Wildside PWA data model](wildside-pwa-data-model.md) – mockup-derived,
  backend-compatible entity shapes for the PWA. _Audience: frontend and backend
  developers._
- [Wildside UX state graph](wildside-ux-state-graph-v0.1.json) –
  machine-readable map of routes, states, transitions, API contracts, and test
  recommendations. _Audience: frontend developers and QA engineers._
- [Wildside PWA sitemap](sitemap.md) – route structure, navigation groups, and
  user flows for the PWA. _Audience: frontend developers and UX reviewers._
- [v2a front-end stack](v2a-front-end-stack.md) – Bun, Vite, React, TanStack,
  Tailwind, DaisyUI, Radix, i18n, map, and testing stack reference. _Audience:
  frontend developers._
- [Data model-driven card architecture](data-model-driven-card-architecture.md)
  – entity-owned localization and SI-unit card modelling. _Audience: frontend
  developers and data model maintainers._
- [Architecting resilient local-first applications in React](local-first-react.md)
  – strategies for offline-first apps using Zustand and TanStack Query.
  _Audience: frontend developers._
- [High-velocity, accessibility-first component testing](high-velocity-accessibility-first-component-testing.md)
  – Vitest and Playwright strategy for accessible components. _Audience:
  frontend developers and QA engineers._
- [Building accessible and responsive progressive web applications](building-accessible-and-responsive-progressive-web-applications.md)
  – standards-focused guide to PWA installability, service workers, responsive
  design, and WCAG. _Audience: frontend developers and QA engineers._
- [React and Tailwind with Bun](react-tailwind-with-bun.md) – Bun-centric React,
  Tailwind, Vite, and static preview reference. _Audience: frontend developers._
- [Tailwind and DaisyUI upgrade](tailwind-and-daisyui-upgrade.md) – tracked work
  item for aligning the PWA workspace with Tailwind v4 and DaisyUI v5.
  _Audience: frontend developers and contributors._
- [Tailwind v3 to v4 migration guide](tailwind-v3-v4-migration-guide.md) –
  migration notes for Tailwind CSS v4’s CSS-first configuration and breaking
  changes. _Audience: frontend developers._
- [Tailwind CSS v4 guide](tailwind-v4-guide.md) – Tailwind v4 setup, custom
  utilities, variants, theming, and utility reference. _Audience: frontend
  developers._
- [daisyUI v5 guide](daisyui-v5-guide.md) – DaisyUI v5 installation,
  configuration, theme roles, and component class reference. _Audience: frontend
  developers._
- [Semantic Tailwind with daisyUI best practice](semantic-tailwind-with-daisyui-best-practice.md)
  – semantic HTML, Radix state styling, Tailwind utilities, DaisyUI roles, and
  token guidance. _Audience: frontend developers._
- [Enforcing semantic Tailwind best practice](enforcing-semantic-tailwind-best-practice.md)
  – Biome, GritQL, Semgrep, and Stylelint rules for semantic, token-driven
  markup. _Audience: frontend developers and tooling maintainers._

## Rust testing practices

- [Reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md)
  – using the `mockable` crate for deterministic tests. _Audience: Rust
  developers._
- [Guide to ergonomic and DRY Rust doctests](rust-doctest-dry-guide.md) –
  patterns for maintainable doctests. _Audience: Rust developers._
- [Mastering test fixtures in Rust with `rstest`](rust-testing-with-rstest-fixtures.md)
  – fixture and parameterized testing techniques. _Audience: Rust developers._

## Infrastructure and delivery

- Infrastructure automation, GitOps workflows, and ephemeral preview
  environments are documented in the Nile Valley repository
  (`../../nile-valley`). This repository keeps the application code, container
  images, Helm chart, and developer-local k3d preview helper that Nile Valley
  deploys or exercises.

## Operational runbooks

- [Wildside server users guide](users-guide.md) – user-visible backend API
  behaviour, including users list pagination errors. _Audience: API consumers
  and backend developers._
- [OSM ingestion end-to-end runbook](runbooks/osm-ingestion-e2e.md) – operator
  procedure for executing and verifying `ingest-osm` runs, including
  deterministic reruns. _Audience: backend operators and developers._
- [Session signing key rotation](runbooks/session-key-rotation.md) – procedure
  for rotating backend session signing keys in Kubernetes. _Audience: platform
  engineers and operators._

## Developer guidelines and tooling

- [Documentation style guide](documentation-style-guide.md) – conventions for
  clear, consistent docs. _Audience: all contributors._
- [PWA assistant instructions](../frontend-pwa/AGENTS.md) – scoped TypeScript,
  JavaScript, testing, and quality-gate guidance for the browser PWA.
  _Audience: frontend developers and automation agents._
- [Package assistant instructions](../packages/AGENTS.md) – scoped TypeScript
  package boundaries, testing, and quality-gate guidance for shared packages.
  _Audience: package maintainers and automation agents._
- [Security assistant instructions](../security/AGENTS.md) – scoped JavaScript
  audit policy, validation, and security automation guidance. _Audience:
  security automation maintainers and automation agents._
- [Scripting standards](scripting-standards.md) – Python-first automation
  guidance covering `uv`, `plumbum`, and testing expectations. _Audience:
  automation authors._
- [Complexity antipatterns and refactoring strategies](complexity-antipatterns-and-refactoring-strategies.md)
  – managing code complexity. _Audience: implementers and maintainers._
- [A command-line wizard’s guide to srgn](srgn.md) – using `srgn` for syntactic
  code refactoring. _Audience: developers performing code changes._
- [Biome configuration schema](biome-schema.json) – JSON schema for
  `biome.json`. _Audience: contributors editing Biome settings._
