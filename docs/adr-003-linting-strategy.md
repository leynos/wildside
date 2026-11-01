# Architecture Decision Record (ADR) 003: Front-end linting strategy

## Status

Accepted — 2026-10-26: add pinned Unicorn and `ai-deslop` Oxlint profiles to
the front-end application.

## Date

2026-10-26

## Context

The Progressive Web Application (PWA) already uses Biome for formatting and
general linting, TypeScript for static type checking, and TypeDoc for public
API documentation. Those checks have distinct jobs, but they do not supply the
Unicorn idiom rules or the evidence-focused rules in `ai-deslop`.

The linter configuration must be reproducible and must not weaken existing
Biome, TypeDoc, or house-rule requirements to make the new checks pass.
Oxlint's JavaScript plugin interface is still in alpha, so its version and its
plugin version need to be reviewed and pinned together.

## Decision

Run Oxlint over `frontend-pwa` as part of `make lint-frontend`. Keep Biome as
the owner of formatting and general linting for `frontend-pwa` and the shared
packages, and keep TypeScript and TypeDoc as the owners of their existing
checks.

Enable Oxlint's built-in `unicorn` plugin with its recommended profile. The
profile's rules are listed explicitly in `frontend-pwa/oxlint.config.ts`, and
all Oxlint categories remain off. This keeps the selected set reviewable and
prevents category defaults from silently changing it.

Load `ai-deslop`'s exported recommended options and rules from the pinned
package. Pin `oxlint` to `1.82.0` and `ai-deslop` to `0.4.9`; the latter uses
`@oxlint/plugins` `1.82.0`, matching the Oxlint runtime. Its recommended
profile enables type-aware rules, so pin `oxlint-tsgolint` to `7.0.2005` as the
required type-aware backend. Lock both package manager lockfiles so local and
CI runs use the same versions.

Disable an individual rule only when it conflicts with an existing Biome or
`df12-lints` requirement, TypeDoc's documentation contract, or a conflicting
Unicorn and `ai-deslop` rule. In the last case, retain Unicorn and disable the
`ai-deslop` rule. Record each exception and its affected code in this ADR. The
`unicorn/filename-case` rule is disabled for `src/Main.tsx` and
`src/app/App.tsx` only. Biome's `useFilenamingConvention` requires PascalCase
for these component exports, which conflicts with Unicorn's kebab-case rule.
The files retain their established names so both tools can continue to enforce
their respective requirements elsewhere.

Remediate findings in source code where both policies can be satisfied. Keep
TypeDoc's warnings-as-errors gate unchanged, and do not add file-wide or
unexplained inline suppressions for either Oxlint plugin.

## Consequences

- `make lint-frontend` now runs Biome and then the pinned Oxlint profile for
  the PWA. The package-local `lint` script runs the same Oxlint configuration.
- A Unicorn, `ai-deslop`, or `oxlint-tsgolint` upgrade requires reviewing the
  recommended profile, updating exact dependency pins and lockfiles, and
  reconciling any new findings before the change is accepted.
- Oxlint JavaScript plugins remain an alpha interface; a future Oxlint upgrade
  must verify the plugin runtime and the complete front-end lint gate.
- The two filename overrides are constrained to components whose exported
  names require PascalCase under Biome.
- Formatting, general lint, type checking, and API documentation remain
  independent gates with their existing owners.

## Alternatives considered

- **Run Oxlint over every workspace.** Rejected: this change targets the PWA;
  shared TypeScript packages retain their current lint ownership until a
  separate adoption decision covers them.
- **Enable broad Oxlint categories.** Rejected: this would add unrelated rule
  families and could duplicate Biome's general lint checks.
- **Use a floating or ranged plugin version.** Rejected: a changed rule set
  must arrive through a reviewed lockfile change.

## References

- [Oxlint built-in plugin guide](https://oxc.rs/docs/guide/usage/linter/plugins.html)
  and
  [JavaScript plugin guide](https://oxc.rs/docs/guide/usage/linter/js-plugins.html).
- [`ai-deslop` repository](https://github.com/maharshi365/deslop), including
  its recommended configuration and rule descriptions.
- [Simulacat Core pull request 121](https://github.com/leynos/simulacat-core/pull/121),
  used as the implementation model.
