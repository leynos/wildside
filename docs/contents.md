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

- [Pure, accessible, and localizable React components](pure-accessible-and-localizable-react-components.md)
  – building accessible, localizable components with Radix, TanStack, and
  DaisyUI. *Audience: frontend developers.*
- [Wildside PWA design](wildside-pwa-design.md) – frontend architecture,
  offline-first behaviour, and localization strategy. *Audience: frontend
  developers and contributors.*
- [Tailwind and DaisyUI upgrade](tailwind-and-daisyui-upgrade.md) – tracked
  work item for aligning the PWA workspace with Tailwind v4 and DaisyUI v5.
  *Audience: frontend developers and contributors.*
- [Wildside PWA data model](wildside-pwa-data-model.md) – mockup-derived,
  backend-compatible entity shapes for the PWA. *Audience: frontend and backend
  developers.*
- [Architecting resilient local-first applications in React](local-first-react.md)
  – strategies for offline-first apps using Zustand and TanStack Query.
  *Audience: frontend developers.*
- [High-velocity, accessibility-first component testing](high-velocity-accessibility-first-component-testing.md)
  – Vitest and Playwright strategy for accessible components. *Audience:
  frontend developers and QA engineers.*

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
