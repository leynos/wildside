# wildside

Wildside is a mobile “serendipity engine” for urban exploration, generating
bespoke, narrative-rich walking tours tailored to user interests, time, and
location. Leveraging OpenStreetMap, Wikidata, and a custom orienteering-based
algorithm, it optimizes for “interestingness” over efficiency. The MVP will be
a PWA with a Rust/React stack, self-hosted map/routing services, and a strong
data-validation pipeline. The strategy emphasises cost control, security-first
AI integration, and clear differentiation from fitness, hiking, and static tour
apps.

## Formatting, linting, and type checking

Use the Makefile targets to format, lint, and type-check both the Rust backend
and the TypeScript/JavaScript workspaces:

```bash
# Install Bun dependencies
make deps

# Format all code (Rust + Biome with write)
make fmt

# Lint all code (Clippy + Biome CI)
make lint

# Type-check TypeScript workspaces
make typecheck

# Check formatting only (no writes)
make check-fmt
```

Under the hood, Biome runs via Bun (see the Makefile). If you prefer to invoke
Biome directly:

```bash
# Format JS/TS files in-place
bun x biome format --write

# Lint with CI output for selected packages/paths
bun x biome ci \
  frontend-pwa \
  packages/tokens/src packages/tokens/build \
  packages/types/src
```

Notes:

- Biome respects `.biomeignore` and VCS ignore files (we enable
  `vcs.useIgnoreFile`), so build artefacts such as any `target/` directory are
  ignored. There is also an explicit override that disables Biome for
  `**/target/**`.
- Run `make deps` once in the repo root if Bun tooling is not already set
  up locally.

## Documentation linting

Ensure documentation and diagrams remain valid:

```bash
make markdownlint-docs
make mermaid-lint
```

## Helm configuration

The included Helm chart surfaces several values for managing secrets:

| Value | Default | Purpose |
| ----- | ------- | ------- |
| `existingSecretName` | `""` | Name of a Secret to source environment variables from. |
| `secretEnvFromKeys` | `{}` | Map environment variables to keys in `existingSecretName`. |
| `allowMissingSecret` | `true` | Permit rendering when the Secret is absent. Set to `false` to fail. |

Note: Helm client version 3.2.0 or later is required when `secretEnvFromKeys`
is used, as the chart invokes `lookup` during template rendering.
