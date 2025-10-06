# Documentation index

## Project architecture

- [Wildside high-level design](wildside-high-level-design.md) – strategic
  blueprint and product vision. *Audience: stakeholders and all
  contributors.*
- [Repository design guide](repository-structure.md) – explains repository
  layout and request flow. *Audience: new contributors.*
- [Wildside backend: functional design specification][backend-spec] –
  outlines backend components and tasks. *Audience: backend developers.*
- [Backend MVP architecture and observability](backend-design.md) – details
  monolithic backend and observability plan. *Audience: backend developers.*
- [Values class diagram](values-class-diagram.mmd) – Mermaid diagram of Helm
  chart values. *Audience: platform engineers.*

## Frontend development

- [Pure, accessible, and localizable React components][pure-react-components]
  – building accessible, localized components with Radix, TanStack, and
  DaisyUI. *Audience: frontend developers.*
- [Architecting resilient local-first applications in React][local-first]
  – strategies for offline-first apps using Zustand and TanStack Query.
  *Audience: frontend developers.*
- [High-velocity, accessibility-first component testing][accessibility-testing]
  – Vitest and Playwright strategy for accessible components.
  *Audience: frontend developers and QA engineers.*

## Rust testing practices

- [Reliable testing in Rust via dependency injection][rust-di] – using the
  `mockable` crate for deterministic tests. *Audience: Rust developers.*
- [Guide to ergonomic and DRY Rust doctests][rust-doctest] – patterns for
  maintainable doctests. *Audience: Rust developers.*
- [Mastering test fixtures in Rust with `rstest`][rust-rstest] – fixture and
  parameterized testing techniques. *Audience: Rust developers.*

## Infrastructure and delivery

- [Cloud-native architecture for preview environments][cloud-previews] –
  GitOps-driven preview platform design. *Audience: platform engineers.*
- [Ephemeral previews infrastructure roadmap](ephemeral-previews-roadmap.md)
  – phased plan for preview environment infrastructure. *Audience: platform
  engineers and project managers.*
- [Architecting a modern CI/CD pipeline](ci-cd-container-pipeline-design.md) –
  GitHub Actions to Kubernetes workflow. *Audience: DevOps engineers.*
- [Declarative DNS guide](declarative-dns-guide.md) –
  automating Cloudflare DNS with FluxCD, ExternalDNS, and OpenTofu. *Audience:
  platform engineers.*
- [Declarative TLS guide](declarative-tls-guide.md) – automating certificate
  management with cert-manager. *Audience: platform engineers.*
- [Using Cloudflare DNS with OpenTofu][cloudflare-opentofu] – practical steps
  for managing DNS records. *Audience: infrastructure developers.*
- [A comprehensive developer’s guide to HCL for OpenTofu][opentofu-hcl] –
  HCL syntax and workflows. *Audience: infrastructure developers.*
- [Unit testing OpenTofu modules and scripts][opentofu-testing] – strategies
  for testing IaC modules. *Audience: infrastructure developers.*
- [Infrastructure test dependency checklist][infra-test-deps]
  – validates CLI prerequisites before running Terraform policy suites.
  *Audience: infrastructure developers and CI engineers.*
- [DOKS OpenTofu module design](doks-module-design.md) – design decisions for
  the DigitalOcean Kubernetes module. *Audience: infrastructure developers.*
- [FluxCD OpenTofu module design](fluxcd-module-design.md) – design decisions for
  the GitOps control plane module. *Audience: infrastructure developers.*
- [Vault appliance OpenTofu module design](vault-appliance-module-design.md) –
  design decisions for the Vault infrastructure module. *Audience:
  infrastructure developers.*

## Developer guidelines and tooling

- [Documentation style guide](documentation-style-guide.md) – conventions for
  clear, consistent docs. *Audience: all contributors.*
- [Scripting standards](scripting-standards.md) – Python-first automation
  guidance covering `uv`, `plumbum`, and testing expectations. *Audience:
  automation authors.*
- [Complexity antipatterns and refactoring strategies][complexity-guide] –
  managing code complexity. *Audience: implementers and maintainers.*
- [A command-line wizard’s guide to srgn](srgn.md) – using `srgn` for
  syntactic code refactoring. *Audience: developers performing code changes.*
- [Biome configuration schema](biome-schema.json) – JSON schema for
  `biome.json`. *Audience: contributors editing Biome settings.*

[backend-spec]: wildside-backend-design.md
[pure-react-components]: pure-accessible-and-localizable-react-components.md
[local-first]: local-first-react.md
[accessibility-testing]: high-velocity-accessibility-first-component-testing.md
[rust-di]: reliable-testing-in-rust-via-dependency-injection.md
[rust-doctest]: rust-doctest-dry-guide.md
[rust-rstest]: rust-testing-with-rstest-fixtures.md
[cloud-previews]: cloud-native-ephemeral-previews.md
[cloudflare-opentofu]: using-cloudflare-dns-with-opentofu.md
[infra-test-deps]: infrastructure-test-dependencies.md
[opentofu-hcl]: opentofu-hcl-syntax-guide.md
[opentofu-testing]: opentofu-module-unit-testing-guide.md
[complexity-guide]: complexity-antipatterns-and-refactoring-strategies.md
