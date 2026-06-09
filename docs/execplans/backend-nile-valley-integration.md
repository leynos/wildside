# Integrate Wildside with Nile Valley previews

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This plan was approved for implementation on 2026-05-21.

## Purpose / big picture

Wildside already has a partial deployment surface: an Actix Web runtime,
`/health/live` and `/health/ready` endpoints, a backend container image, and a
Helm chart under `deploy/charts/wildside`. The current surface is not yet fully
aligned with the Nile Valley preview and GitOps contract. The main gaps are
that health semantics live in the inbound HTTP adapter instead of a
domain-owned port, the chart lacks the full ExternalSecret and local preview
contract used by Corbusier and Nile Valley, and this repository has no local
`k3d` orchestration workflow.

After this change, a developer can run the Wildside backend as the production
HTTP entry point, build a non-root container image with stable health probes,
render and install a Nile Valley-compatible Helm chart, and use Makefile
targets to create, inspect, tail, and tear down a local `k3d` preview
environment. A reviewer can observe success by requesting `/health/live` and
`/health/ready`, rendering the Helm chart with local and GitOps-style values,
and running the full repository gates.

Observable success criteria:

- `backend/src/domain` owns a health observation port and default
  implementation, with no Actix Web or Kubernetes types in the domain layer.
- `backend/src/inbound/http/health.rs` maps the domain health result to
  Actix Web responses for `GET /health/live` and `GET /health/ready`.
- `backend/src/main.rs` and `backend/src/server/*` keep the HTTP server as the
  Wildside runtime entry point.
- `deploy/docker/backend.Dockerfile` and the repository `.dockerignore` build a
  release backend image, run it as a non-root user, expose the configured HTTP
  port, and probe `/health/live` or `/health/ready` consistently.
- `deploy/charts/wildside` supports configurable ingress, non-secret config,
  Secret-derived environment variables, session Secret mounting, and
  ExternalSecret rendering compatible with Nile Valley.
- `scripts/local_k8s.py`, `scripts/local_k8s/*`, and `make local-k8s-*`
  provide a Python, Cyclopts, and `uv` driven local `k3d` preview workflow.
- `docs/local-k8s-preview-design.md`, this ExecPlan, `docs/users-guide.md`,
  `docs/developers-guide.md`, `docs/wildside-backend-architecture.md`, and
  `docs/backend-roadmap.md` describe the user-facing and internal contracts.
- Focused `rstest` unit tests and `rstest-bdd` behavioural tests cover healthy
  and unhealthy health observations plus relevant adapter and command-line
  behaviour.
- `make check-fmt`, `make lint`, and `make test` pass, with command output
  retained in `/tmp` logs.

## Constraints

- Do not implement this plan until the user explicitly approves it.
- Preserve the hexagonal dependency rule from the `hexagonal-architecture`
  skill and `docs/wildside-backend-architecture.md`: dependencies point inward,
  the domain defines ports, and adapters implement or consume them.
- Keep health domain code free of Actix Web, Kubernetes, Docker, Helm, or
  environment-variable types.
- Keep the inbound HTTP adapter thin: parse no business policy there, and map
  only domain health observations to HTTP status codes and headers.
- Use the existing Actix Web runtime as the server entry point. Do not add a
  second long-running process or sidecar-specific health server.
- Prefer extending the existing backend Dockerfile and Helm chart over adding
  duplicate deployment assets, unless validation proves the existing assets are
  beyond repair.
- Align with the Corbusier Nile Valley integration where it fits Wildside:
  health endpoints, startup probe semantics, ExternalSecret support,
  loopback-bound `k3d` ingress, Cyclopts CLI shape, and Makefile target names.
- Keep local preview automation repository-local and developer-focused. The
  Nile Valley repository remains the owner of shared preview infrastructure and
  GitOps automation.
- Use Makefile targets for validation. Run tests, lints, and format checks
  sequentially, with `tee` writing logs under `/tmp`.
- Do not run sub-agent tests. Sub-agents may inspect and summarize, but the
  coordinator owns all gate runs.
- Keep source files under 400 lines. Split new Python and Rust modules by
  feature when needed.
- New Rust modules must begin with `//!` module documentation. Public Rust APIs
  need Rustdoc with examples when the examples add useful information.
- Documentation uses en-GB-oxendict spelling and follows
  `docs/documentation-style-guide.md`.
- Update `docs/contents.md` when new long-lived documentation is added.
- Use `coderabbit review --agent` after each major implementation milestone and
  resolve concerns before moving to the next milestone.
- Commit frequently after gated, atomic changes. Use file-based commit messages
  through `git commit -F`, never `git commit -m`.

## Tolerances

- Scope tolerance: if implementation requires replacing Actix Web, splitting
  the backend into multiple deployable services, or changing public API paths
  beyond `/health/live` and `/health/ready`, stop and request approval.
- Architecture tolerance: if a domain module needs to import Actix Web,
  Kubernetes, Helm, Docker, `plumbum`, or other infrastructure types, stop and
  redesign the port boundary before continuing.
- Chart tolerance: if the Wildside chart cannot be made compatible with Nile
  Valley without breaking existing `deploy/docker-compose.yml` or documented
  chart values, stop and record options in the Decision Log.
- Local preview tolerance: if a full local preview needs cluster-admin actions
  outside `k3d`, `kubectl`, Helm, CloudNativePG, or Valkey operator
  installation, stop and ask whether that belongs in Nile Valley instead.
- Dependency tolerance: adding `cyclopts` and `plumbum` as inline `uv` script
  dependencies is expected. Adding more than two new Rust production
  dependencies or more than four Python dependencies requires explicit approval.
- Test tolerance: if `rstest-bdd` cannot reasonably exercise a network or CLI
  boundary without excessive runtime, add a narrower behavioural test and
  document the reason. Do not add superficial BDD scenarios.
- Proof tolerance: no Verus, Kani, or proptest work is expected for this
  feature because the planned logic is configuration, boundary mapping, and
  orchestration rather than a broad state-space invariant. If implementation
  introduces non-trivial port-allocation, state-transition, or retry
  invariants, revisit this decision.
- Gate tolerance: after three repair loops on the same gate failure, stop,
  record the failing command and log path, and ask for direction.
- Environment tolerance: if `k3d`, Docker, `kubectl`, Helm, `coderabbit`, or
  local networking are unavailable, document the blocker and validate the
  nearest render or unit-test substitute instead of faking success.

## Risks

- Risk: Wildside already has health endpoints, but their semantics are owned by
  `backend/src/inbound/http/health.rs`. Severity: medium. Likelihood: high.
  Mitigation: move policy into a domain-owned health module and keep the HTTP
  adapter as response mapping only.

- Risk: the current backend Dockerfile probes `/health`, while the server and
  chart use `/health/live` and `/health/ready`. Severity: high. Likelihood:
  high. Mitigation: standardize probes on `/health/live` for liveness/startup
  and `/health/ready` for readiness, matching the Corbusier follow-up decision.

- Risk: the Wildside chart currently resembles Nile Valley's generic
  `example-app` chart more than Corbusier's hardened chart. Severity: medium.
  Likelihood: high. Mitigation: port only the chart contract that Nile Valley
  expects: ExternalSecret, schema validation, Secret lookup controls, service
  account, ingress hosts, and stable probe schema.

- Risk: local `k3d` orchestration may overlap with Nile Valley ownership.
  Severity: medium. Likelihood: medium. Mitigation: keep the repository-local
  workflow as a developer preview that builds Wildside's image and installs
  Wildside's chart, while documenting that multi-application GitOps automation
  remains in Nile Valley.

- Risk: a full local preview may be slow or flaky because it depends on Docker,
  `k3d`, Kubernetes controllers, CloudNativePG, and Valkey. Severity: medium.
  Likelihood: medium. Mitigation: unit-test helper parsing and validation
  logic, use bounded waits, surface clear preflight errors, and reserve
  end-to-end preview execution for explicit local validation.

- Risk: documentation requirements mention `docs/users-guide.md`, but this
  repository currently uses component-specific user guides instead of that
  global file. Severity: low. Likelihood: high. Mitigation: create
  `docs/users-guide.md` as the user-facing entry point for server and preview
  behaviour, and link it from `docs/contents.md`.

## Relevant skills and documentation

Use these skills during implementation:

- `leta`: navigate code symbols, references, call hierarchy, and refactors.
- `rust-router`: route Rust design issues to smaller Rust skills as needed.
- `hexagonal-architecture`: keep health policy in the domain and transport
  concerns in adapters.
- `execplans`: keep this plan current throughout implementation.
- `firecrawl-mcp`: refresh external prior art only when local context is
  insufficient or external conventions may have changed.
- `commit-message`: commit with a file-based message after gated changes.

Signpost these repository documents while working:

- `AGENTS.md`
- `docs/wildside-backend-architecture.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rstest-bdd-users-guide.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/pg-embed-setup-unpriv-users-guide.md`
- `docs/documentation-style-guide.md`
- `docs/developers-guide.md`
- `docs/backend-roadmap.md`
- `docs/execplans/remove-ephemeral-previews.md`
- `docs/repository-structure.md`

Planning used a Wyvern agent team and context pack
`wildside-nile-valley-planning` (`pk_tz64s5e4`) to share code references for
the existing health adapter, server bootstrap, and Helm values.

## Prior art and external references

Firecrawl was used to resolve gaps around Corbusier and Nile Valley. The
implementation should keep these references nearby but must still adapt them to
Wildside's existing structure.

- Corbusier Makefile:
  <https://github.com/leynos/corbusier/blob/main/Makefile>
- Corbusier local preview CLI:
  <https://raw.githubusercontent.com/leynos/corbusier/main/scripts/local_k8s.py>
- Corbusier local preview orchestration:
  <https://raw.githubusercontent.com/leynos/corbusier/main/scripts/local_k8s/orchestration.py>
- Corbusier local preview config:
  <https://raw.githubusercontent.com/leynos/corbusier/main/scripts/local_k8s/config.py>
- Corbusier chart values and ExternalSecret template:
  <https://raw.githubusercontent.com/leynos/corbusier/main/charts/corbusier/values.yaml>
  and
  <https://raw.githubusercontent.com/leynos/corbusier/main/charts/corbusier/templates/externalsecret.yaml>
- Nile Valley repository overview:
  <https://github.com/leynos/nile-valley>
- Nile Valley example chart values:
  <https://raw.githubusercontent.com/leynos/nile-valley/main/deploy/charts/example-app/values.yaml>

The Corbusier implementation records several useful decisions: use
`/health/live` for startup probes, keep local ingress bound to `127.0.0.1`,
retry `k3d` cluster creation on loopback port collisions, make Helm Secret
lookup opt-in for offline renders, and let ExternalSecret provide the effective
Secret name when `existingSecretName` is unset.

## Current repository orientation

The existing runtime and deployment surface is spread across these files:

- `backend/src/main.rs` wires runtime configuration and starts the backend.
- `backend/src/server/mod.rs` builds the Actix Web application and marks the
  current health state ready after binding the server.
- `backend/src/server/config.rs` and
  `backend/src/server/state_builders.rs` own runtime configuration and service
  composition.
- `backend/src/inbound/http/health.rs` currently owns both health state and
  Actix Web endpoint mapping.
- `backend/src/doc.rs` registers health endpoints in OpenAPI.
- `deploy/docker/backend.Dockerfile` builds and runs the backend image.
- `deploy/docker-compose.yml` uses the backend health endpoint locally.
- `deploy/charts/wildside/*` contains the Helm chart, values, templates, and
  schema.
- `Makefile` defines the quality gates and currently lacks `local-k8s-*`
  targets.

The implementation should first verify these files rather than assuming they
are absent.

## Implementation plan

### Milestone 0: Baseline and plan approval

Do not edit implementation files in this milestone. Confirm the branch, check
for a clean or understood worktree, and keep this ExecPlan in `DRAFT` until the
user approves it.

Run:

```bash
git branch --show-current
git status --short
leta workspace info
```

Expected result: the branch is not `main`, no unrelated dirty changes are
mistaken for implementation work, and Leta reports this worktree as the active
workspace.

Once the plan is approved, change `Status` to `IN PROGRESS`, record the
approval in `Progress`, and continue.

### Milestone 1: Establish failing health tests

Add focused tests before changing production health code.

Create or extend tests so they prove:

- a default health observer starts live but not ready;
- marking readiness makes readiness report healthy;
- marking liveness unhealthy makes liveness report unhealthy;
- the HTTP readiness route returns `200` only for ready observations;
- the HTTP liveness route returns `200` for live observations and `503` for
  unhealthy observations;
- all health probe responses include `Cache-Control: no-store`.

Use `rstest` for domain and adapter unit tests. Add `rstest-bdd` only where it
captures externally observable behaviour more clearly than a direct Actix test,
for example a feature describing probe responses across ready and not-ready
states.

Expected initial result: at least one new test fails because health semantics
are not yet domain-owned.

### Milestone 2: Move health semantics behind a domain port

Introduce a domain health module and port without leaking infrastructure:

- Add `backend/src/domain/health.rs` with health status/value types such as
  `HealthStatus`, `HealthObservation`, and a default implementation suitable
  for process readiness and liveness.
- Add or extend `backend/src/domain/ports/*` with a `HealthObserver` or
  equivalent trait that returns domain-owned observations.
- Export the module through `backend/src/domain/mod.rs` and
  `backend/src/domain/ports/mod.rs`.
- Refactor `backend/src/inbound/http/health.rs` so Actix Web receives an
  injected health observer/state and maps it to HTTP only.
- Update `backend/src/server/mod.rs` and related wiring so readiness is marked
  after the server is constructed, as today, but through the domain-owned
  abstraction.
- Preserve OpenAPI registration in `backend/src/doc.rs`.

The domain module may use standard library synchronization primitives if the
runtime needs shared mutable process state. It must not know about HTTP,
Kubernetes probes, or container health checks.

Run the targeted health tests. Then run:

```bash
action=health-unit
branch="$(git branch --show-current | tr '/ ' '--')"
project="$(basename "$(git rev-parse --show-toplevel)")"
cargo test --manifest-path backend/Cargo.toml health 2>&1 | tee "/tmp/${action}-${project}-${branch}.out"
```

Commit this milestone only after the targeted tests pass. Run
`coderabbit review --agent` and resolve concerns before continuing.

### Milestone 3: Harden the container image

Audit and update the container build surface:

- Add a repository `.dockerignore` if one is absent.
- Update `deploy/docker/backend.Dockerfile` to use a current Rust toolchain
  compatible with this repository.
- Keep the image multi-stage.
- Build the release backend binary with locked dependencies.
- Run as a non-root user with a stable numeric UID/GID.
- Install only runtime packages required by the final binary and health probe.
- Expose the server port, defaulting to `8080`.
- Set `RUST_LOG=info`.
- Configure the Docker health check to probe `/health/live` by default.
- Ensure the health path and port agree with Helm and docker-compose.

Validate with a local image build when Docker is available:

```bash
action=docker-build
branch="$(git branch --show-current | tr '/ ' '--')"
project="$(basename "$(git rev-parse --show-toplevel)")"
docker build -f deploy/docker/backend.Dockerfile -t wildside-backend:local . 2>&1 | tee "/tmp/${action}-${project}-${branch}.out"
```

If Docker is unavailable, record the preflight failure and still run any
available Dockerfile lint or static validation. Commit the container changes
only after validation. Run `coderabbit review --agent` and resolve concerns.

### Milestone 4: Align the Helm chart with Nile Valley

Harden `deploy/charts/wildside` using Corbusier and Nile Valley as prior art.

Required chart work:

- Add or complete `templates/externalsecret.yaml`.
- Add `externalSecret.enabled`, `refreshInterval`, `secretStoreRef`,
  `targetName`, and `data` values.
- Add `validateExistingSecret` so offline renders do not require a live
  cluster lookup.
- Resolve the effective Secret name from `existingSecretName` or the
  ExternalSecret target default.
- Keep `allowMissingSecret` behaviour explicit and schema-validated.
- Add or verify service account support if Nile Valley values expect it.
- Support ingress hosts, paths, annotations, class name, and TLS in a way that
  covers both hostless local ingress and GitOps overlays.
- Use `/health/live` for liveness and startup probes, and `/health/ready` for
  readiness.
- Make config checksum annotations conditional so empty config renders cleanly.
- Keep Pod Security Context and container security context non-root and
  restricted.
- Update `values.schema.json` to reject malformed probe, Secret, ingress, and
  ExternalSecret values.
- Add `values.local.yaml` if the local preview workflow needs local image and
  hostless ingress overrides.

Validate with Helm:

```bash
action=helm-template
branch="$(git branch --show-current | tr '/ ' '--')"
project="$(basename "$(git rev-parse --show-toplevel)")"
helm lint deploy/charts/wildside 2>&1 | tee "/tmp/${action}-${project}-${branch}.out"
helm template wildside deploy/charts/wildside 2>&1 | tee -a "/tmp/${action}-${project}-${branch}.out"
```

If a local values file is added, also render it:

```bash
helm template wildside deploy/charts/wildside \
  --values deploy/charts/wildside/values.local.yaml \
  2>&1 | tee -a "/tmp/${action}-${project}-${branch}.out"
```

Commit chart changes after the render checks pass. Run
`coderabbit review --agent` and resolve concerns.

### Milestone 5: Add local `k3d` orchestration

Add the developer preview workflow modelled on Corbusier but adapted to
Wildside's existing deployment assets.

Create:

- `scripts/local_k8s.py` as the Cyclopts CLI entry point using inline `uv`
  metadata.
- `scripts/local_k8s/config.py` for default names and paths.
- `scripts/local_k8s/validation.py` for executable checks, port validation, and
  local errors.
- `scripts/local_k8s/k3d.py` for cluster creation, deletion, image import,
  kubeconfig environment, and loopback ingress-port discovery.
- `scripts/local_k8s/k8s.py` for namespace and Kubernetes helper operations.
- `scripts/local_k8s/deployment.py` for Docker build, Secret creation, Helm
  install or upgrade, status, and logs.
- Optional `scripts/local_k8s/cnpg.py` and `scripts/local_k8s/valkey.py` if the
  preview provisions PostgreSQL and Valkey locally rather than relying on
  caller-provided URLs.
- Unit tests under the existing Python test layout, or a new `tests/` layout if
  this repository has no Python script tests yet.

The CLI must provide:

- `up`
- `down`
- `status`
- `logs`

The CLI should accept environment-variable overrides with a `WILDSIDE_` prefix
for cluster name, namespace, and ingress port. It should expose a
`--skip-build` option for `up` and `--follow` for `logs`.

Add Makefile targets:

- `local-k8s-up`
- `local-k8s-down`
- `local-k8s-status`
- `local-k8s-logs`

The targets should call `uv run scripts/local_k8s.py ...`. The helper must
print clear preflight errors when `k3d`, `kubectl`, Helm, Docker, or required
controllers are unavailable.

Validate the CLI without creating a cluster first:

```bash
action=local-k8s-help
branch="$(git branch --show-current | tr '/ ' '--')"
project="$(basename "$(git rev-parse --show-toplevel)")"
uv run scripts/local_k8s.py --help 2>&1 | tee "/tmp/${action}-${project}-${branch}.out"
uv run scripts/local_k8s.py up --help 2>&1 | tee -a "/tmp/${action}-${project}-${branch}.out"
make local-k8s-status 2>&1 | tee -a "/tmp/${action}-${project}-${branch}.out"
```

When local tooling is available, validate the full preview:

```bash
action=local-k8s-preview
branch="$(git branch --show-current | tr '/ ' '--')"
project="$(basename "$(git rev-parse --show-toplevel)")"
make local-k8s-up 2>&1 | tee "/tmp/${action}-${project}-${branch}.out"
make local-k8s-status 2>&1 | tee -a "/tmp/${action}-${project}-${branch}.out"
curl -fsS "http://127.0.0.1:${WILDSIDE_K3D_PORT:-<printed-port>}/health/live"
make local-k8s-down 2>&1 | tee -a "/tmp/${action}-${project}-${branch}.out"
```

Replace `<printed-port>` with the port printed by the CLI if no explicit port
was supplied. If a preview fails because local infrastructure is unavailable,
record the precise blocker and keep the non-cluster tests green.

Commit local orchestration changes after validation. Run
`coderabbit review --agent` and resolve concerns.

### Milestone 6: Documentation and roadmap

Add and update documentation while the implementation details are fresh:

- Add `docs/local-k8s-preview-design.md` covering the architecture of the
  local preview workflow, Nile Valley boundaries, container contract, Helm
  values, and expected operator dependencies.
- Add or update `docs/users-guide.md` with user-facing server behaviour:
  health endpoints, container defaults, Helm deployment values, and local
  preview commands.
- Update `docs/developers-guide.md` with internal conventions for health
  ports, chart validation, and local preview helper maintenance.
- Update `docs/wildside-backend-architecture.md` to document the domain health
  observation port and the inbound HTTP adapter mapping.
- Update `docs/repository-structure.md` if new script and chart files change
  the repository layout.
- Update `docs/contents.md` with any new long-lived documents.
- Add an Architectural Decision Record only if implementation makes a
  long-lived architectural choice not adequately captured by the design doc. If
  needed, use the next `docs/adr-NNN-*.md` number after
  `docs/adr-001-websockets-on-actix-ws.md`.
- Update `docs/backend-roadmap.md` by adding a deployment coordination task
  under section 7 if no suitable task exists. Mark it done only after all gates
  pass.

Run documentation validation:

```bash
action=docs
branch="$(git branch --show-current | tr '/ ' '--')"
project="$(basename "$(git rev-parse --show-toplevel)")"
make fmt 2>&1 | tee "/tmp/${action}-${project}-${branch}.out"
make markdownlint 2>&1 | tee -a "/tmp/${action}-${project}-${branch}.out"
make nixie 2>&1 | tee -a "/tmp/${action}-${project}-${branch}.out"
```

Commit documentation changes after validation. Run `coderabbit review --agent`
and resolve concerns.

### Milestone 7: Full gates and closeout

Run the full repository gates sequentially:

```bash
branch="$(git branch --show-current | tr '/ ' '--')"
project="$(basename "$(git rev-parse --show-toplevel)")"

make check-fmt 2>&1 | tee "/tmp/check-fmt-${project}-${branch}.out"
make lint 2>&1 | tee "/tmp/lint-${project}-${branch}.out"
make test 2>&1 | tee "/tmp/test-${project}-${branch}.out"
```

If all gates pass, update this ExecPlan:

- set `Status: COMPLETE`;
- complete the relevant `Progress` entries;
- record final gate evidence and log paths;
- fill in `Outcomes & Retrospective`;
- ensure the relevant roadmap entry is marked done.

Commit the closeout documentation. Run a final `coderabbit review --agent` and
resolve concerns before declaring the work complete.

## Test strategy

Domain tests should cover health status values and transitions without Actix,
networking, Docker, or Kubernetes. Use `rstest` fixtures for repeated initial
state and status cases.

Adapter tests should use Actix Web test utilities to exercise the health routes
with injected domain health observers. These tests prove status-code and header
mapping without binding a real socket.

Behavioural tests with `rstest-bdd` should be used where they describe
externally observable contracts: readiness before and after startup, liveness
after unhealthy marking, and local preview CLI preflight behaviour if the CLI
surface is easier to understand as Given/When/Then scenarios.

Python helper tests should cover pure parsing and validation: port bounds,
`k3d` JSON shape handling, loopback ingress extraction, command construction,
and missing-tool errors. Avoid requiring a live cluster for unit tests.

End-to-end validation should run the local preview only when Docker, `k3d`,
`kubectl`, Helm, and local ports are available. The E2E proof is the rendered
or installed chart plus successful HTTP requests to the health endpoints.

No property tests, Kani harnesses, or Verus proofs are planned initially. If
implementation introduces non-trivial invariants over retry sequences, port
selection, or state transitions, add a Decision Log entry and expand the test
strategy before continuing.

## Progress

- [x] 2026-05-21: Loaded `leta`, `rust-router`, `hexagonal-architecture`,
  `execplans`, `firecrawl-mcp`, and `commit-message` skills for planning.
- [x] 2026-05-21: Created a Leta workspace for this worktree and confirmed
  `leta workspace info` resolves it.
- [x] 2026-05-21: Used a Wyvern agent team to inspect runtime structure,
  testing constraints, roadmap conventions, and documentation touchpoints.
- [x] 2026-05-21: Created context pack
  `wildside-nile-valley-planning` (`pk_tz64s5e4`) for agent-team code
  references.
- [x] 2026-05-21: Used Firecrawl to inspect Corbusier and Nile Valley prior
  art for Makefile targets, local `k3d` orchestration, chart values,
  ExternalSecret support, and Nile Valley chart expectations.
- [x] 2026-05-21: Drafted this ExecPlan for approval.
- [x] 2026-05-21: Ran `make fmt`, `make markdownlint`, and
  `make check-fmt` against the draft plan.
- [x] 2026-05-21: Attempted `coderabbit review --agent` twice for the planning
  milestone; both attempts were blocked by the external CodeRabbit usage rate
  limit before any review findings were returned.
- [x] 2026-05-21: Received explicit user approval to proceed with
  implementation as set out in this ExecPlan.
- [x] 2026-05-21: Recorded approval and set `Status: IN PROGRESS`.
- [x] 2026-05-21: Established health unit and BDD coverage for domain state,
  Actix probe mapping, unhealthy liveness, unready readiness, and
  `Cache-Control: no-store`.
- [x] 2026-05-21: Moved health semantics into a domain-owned
  `ProcessHealth` implementation and `HealthObserver` port, keeping the Actix
  adapter as HTTP response mapping.
- [x] 2026-05-21: Ran health milestone gates successfully:
  `make check-fmt`, `make lint`, and `make test`.
- [x] 2026-05-21: Committed the health milestone as
  `4e3083f Move health observation into domain`.
- [x] 2026-05-21: Ran `coderabbit review --agent` for the health milestone;
  CodeRabbit completed with zero findings.
- [x] 2026-05-21: Hardened the backend container image definition by moving
  to an edition-2024-capable Rust builder, a Debian slim runtime, a non-root
  UID/GID, explicit runtime libraries, `HOST`/`PORT` defaults, and
  `/health/live` as the image liveness check.
- [x] 2026-05-21: Added a root `.dockerignore` so local build artefacts,
  VCS metadata, frontend output, and dependency trees are excluded from image
  contexts.
- [x] 2026-05-21: Ran container milestone gates successfully:
  `make check-fmt`, `make lint`, and `make test`; the Docker image build
  remains blocked locally because Docker is not installed in this environment.
- [x] 2026-05-21: Committed the container milestone as
  `4b41354 Harden backend container image`.
- [x] 2026-05-21: Ran `coderabbit review --agent` for the container
  milestone; CodeRabbit completed with zero findings.
- [x] 2026-05-21: Aligned the Helm chart with Nile Valley conventions:
  ExternalSecret rendering, effective Secret name resolution, optional live
  Secret validation, service account support, hostless/local ingress values,
  schema validation, and `/health/live` startup probes.
- [x] 2026-05-21: Validated the Helm milestone with `helm lint`,
  `helm template --kube-version 1.31.0`, local values rendering, and an
  ExternalSecret render in
  `/tmp/helm-template-wildside-backend-nile-valley-integration.out`.
- [x] 2026-05-21: Committed the Helm milestone as
  `66cf831 Align Helm chart with Nile Valley`.
- [x] 2026-05-21: Ran `coderabbit review --agent` for the Helm milestone;
  CodeRabbit completed with zero findings.
- [x] 2026-05-21: Added local `k3d` orchestration scaffolding with a
  Cyclopts CLI, Makefile targets, configuration, validation, k3d, Kubernetes,
  and Helm deployment helpers.
- [x] 2026-05-21: Validated the local preview CLI help and pure Python
  validation tests; `make local-k8s-status` now reports the expected local
  blocker because `k3d` and `kubectl` are not installed in this environment.
- [x] 2026-05-21: Ran local preview milestone gates successfully:
  `make check-fmt`, `make lint`, and `make test`.
- [x] 2026-05-21: Committed the local preview milestone as
  `5aaf44f Add local k3d preview workflow`.
- [x] 2026-05-21: Ran `coderabbit review --agent` for the local preview
  milestone; CodeRabbit completed with zero findings.
- [x] 2026-05-21: Updated design, user, developer, architecture, contents,
  repository-structure, and roadmap docs for the local preview and Nile Valley
  integration contracts.
- [x] 2026-05-21: Ran documentation validation successfully: `make fmt`,
  `make markdownlint`, and `make nixie`.
- [x] 2026-05-21: Committed the documentation milestone as
  `fea1a1d Document Nile Valley preview integration`.
- [x] 2026-05-21: Ran `coderabbit review --agent` for the documentation
  milestone; CodeRabbit completed with zero findings.
- [x] 2026-05-21: Ran final gates successfully: `make check-fmt`,
  `make lint`, and `make test`.
- [x] 2026-05-21: Closed this ExecPlan after all implementation, validation,
  documentation, roadmap, and review requirements were satisfied except for
  environment-blocked Docker/k3d execution.

## Surprises & discoveries

- Wildside already has `backend/src/inbound/http/health.rs` with
  `/health/live` and `/health/ready`, so the implementation is a refactor and
  hardening task rather than a fresh endpoint addition.
- Wildside already has `deploy/docker/backend.Dockerfile` and
  `deploy/charts/wildside`, but the Docker health check currently targets
  `/health` and the chart is missing Corbusier-style ExternalSecret support.
- No repository-local `k3d` preview workflow exists today. Existing docs say
  preview infrastructure ownership moved to Nile Valley, so the new local
  workflow must be documented as developer preview tooling rather than shared
  GitOps ownership.
- The repository does not currently contain `docs/users-guide.md`, despite the
  requested update. Creating it is part of this plan.
- `docs/contents.md` may contain stale references; documentation updates should
  reconcile new links carefully without broad housekeeping.
- `coderabbit review --agent` can return exit code 0 while reporting a
  recoverable rate-limit error in its JSON output. The planning milestone could
  not obtain CodeRabbit findings after two attempts on 2026-05-21.
- `rstest-bdd` async scenarios need an explicit async test runtime. The health
  probe BDD test uses `#[tokio::test(flavor = "current_thread")]` so async
  Actix route tests do not attempt to start a nested runtime.
- Docker is not installed in this environment. The container milestone cannot
  run
  `docker build -f deploy/docker/backend.Dockerfile -t wildside-backend:local .`
  here; validation must rely on static review and the repository gates until a
  Docker-enabled host runs the image build.
- This Helm binary defaults `helm template` capabilities to Kubernetes v1.20,
  while the chart declares `kubeVersion: >=1.26.0-0 <1.32.0-0`; Helm renders
  need `--kube-version 1.31.0` in this environment.
- Local preview cluster validation cannot create or inspect a cluster in this
  environment because `k3d` and `kubectl` are not installed. The CLI preflight
  now reports that blocker concisely.
- `make nixie` runs `bun install`, which attempted to refresh `bun.lock` for
  the unrelated `ip-address` override. That lockfile change was excluded from
  the documentation milestone.

## Decision Log

- 2026-05-21: Treat this as a hardening and alignment project, not a greenfield
  add. Rationale: the current repository already contains an Actix runtime,
  health routes, Dockerfile, and Helm chart. Replacing them wholesale would
  increase risk and violate the local preference to extend existing patterns.

- 2026-05-21: Use `/health/live` for liveness and startup probes and
  `/health/ready` for readiness. Rationale: this matches Kubernetes probe
  semantics and the Corbusier follow-up decision discovered via Firecrawl.

- 2026-05-21: Create a domain-owned health observation port before changing the
  HTTP adapter. Rationale: the requested architecture uses hexagonal
  boundaries, and health policy currently lives in the inbound adapter.

- 2026-05-21: Plan to create `docs/users-guide.md`.
  Rationale: the user explicitly requested that file, and the repository does
  not currently have a global user guide covering server and deployment
  behaviour.

- 2026-05-21: Do not plan Verus, Kani, or proptest work for the initial scope.
  Rationale: the planned changes are boundary mapping and orchestration, not a
  broad algorithmic state space. This decision must be revisited if the
  implementation introduces retry or state invariants that merit stronger
  verification.

- 2026-05-21: Begin implementation with the domain health port and HTTP
  adapter tests. Rationale: this is the architecture-bearing change. Container,
  Helm, and local preview work should depend on stable health semantics rather
  than the current adapter-owned state.

- 2026-05-21: Keep `backend::inbound::http::health::HealthState` as a type
  alias for `backend::domain::ProcessHealth` during the refactor. Rationale:
  existing server wiring and callers can keep their current import path while
  the actual health semantics and state live in the domain layer.

- 2026-05-21: Use a Debian slim runtime image rather than continuing the
  Alpine musl image. Rationale: the backend depends on PostgreSQL, OpenSSL, and
  SQLite-linked crates. A glibc runtime with explicit `libpq5`, `libssl3`, and
  `libsqlite3-0` packages keeps the container build simpler and avoids the
  brittle exact Alpine package pins that were already stale.

- 2026-05-21: Keep live Secret lookup validation opt-in through
  `validateExistingSecret`. Rationale: GitOps, CI, and local preview renders
  must work without access to the target cluster, while operators can still
  request a live lookup when installing against a cluster that already contains
  the Secret.

## Outcomes & Retrospective

Implemented.

Wildside now has a domain-owned health observation model and port, with Actix
Web probe handlers mapping that policy to `/health/live` and `/health/ready`.
The backend container image uses a multi-stage build, a Debian slim non-root
runtime, explicit runtime libraries, stable `HOST`/`PORT` defaults, and a
`/health/live` image health check.

The Helm chart now supports Nile Valley-oriented deployment concerns:
ExternalSecret rendering, effective Secret name resolution, optional live
Secret validation, service account configuration, local and host-based ingress
forms, schema validation, and Kubernetes probes aligned with the runtime health
contract.

The repository now provides `make local-k8s-up`, `make local-k8s-status`,
`make local-k8s-logs`, and `make local-k8s-down` targets backed by a Cyclopts/
`uv` Python helper. The helper preflights Docker, Helm, `k3d`, and `kubectl`;
builds and imports the local backend image; and installs the chart with
`values.local.yaml` when the required tools are present.

Documentation now covers user-facing server and preview behaviour in
`docs/users-guide.md`, the local preview and Nile Valley design in
`docs/local-k8s-preview-design.md`, and internal conventions in the developer,
architecture, repository-structure, contents, and roadmap documents. The
backend roadmap entry for Nile Valley preview and GitOps alignment is marked
done.

Validation completed:

- `make check-fmt`
- `make lint`
- `make test`
- `make fmt`
- `make markdownlint`
- `make nixie`
- `helm lint deploy/charts/wildside`
- `helm template deploy/charts/wildside --kube-version 1.31.0`
- local values and ExternalSecret Helm render checks
- local preview CLI help and Python unit tests
- CodeRabbit reviews for health, container, Helm, local preview, and
  documentation milestones, all with zero findings

Residual environment gaps:

- Docker is not installed in this environment, so the backend image build could
  not be executed here.
- `k3d` and `kubectl` are not installed in this environment, so the full local
  preview cluster lifecycle could not be executed here.

Both gaps are covered by preflight checks and documented as local environment
requirements.
