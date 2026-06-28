# Adapt Wildside local previews for rootless Podman and kind

This ExecPlan is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: APPROVED AND IN IMPLEMENTATION. The user approved implementation on
2026-06-22. Keep this plan current as each milestone lands.

## Purpose / big picture

Wildside already exposes the Nile Valley-facing deployment contract through the
backend container image, the Helm chart under `deploy/charts/wildside`, health
probes, and the repository-local `scripts/local_k8s.py` preview helper. That
preview helper currently assumes Docker plus `k3d`. The Episodic commit
`f19cbcf21ad3ecadf164822218ba8e152b3b68bf` shows how a sibling service was
adapted, so local previews can run on this virtual machine with rootless Podman
plus `kind`.

After this change, a developer on the same VM can run Wildside's local preview
through either Docker plus `k3d` or rootless Podman plus `kind`. The default
workflow remains compatible with the current `make local-k8s-*` commands.
Provider-specific differences are hidden behind the local preview adapter, not
leaked into the backend domain or Helm chart contract.

The main observable success path is:

```bash
WILDSIDE_CONTAINER_ENGINE=podman WILDSIDE_K8S_PROVIDER=kind make local-k8s-up
```

The command creates or reuses a `kind` cluster using Podman's rootless provider,
builds the Wildside backend image with Podman, saves the image to a temporary
archive, loads that archive into `kind`, installs the Helm chart, and prints
the `kubectl port-forward` command needed to open the preview on
`127.0.0.1:${WILDSIDE_K8S_PORT:-8088}`. The existing Docker plus `k3d` path
continues to create a loopback-bound load-balancer port and remains usable with:

```bash
make local-k8s-up
```

## Constraints

- The user explicitly approved implementation on 2026-06-22; continue
  milestone-by-milestone within this plan's tolerances.
- Preserve the hexagonal boundary. The backend domain must not learn about
  Docker, Podman, `k3d`, `kind`, Kubernetes, Helm, or local preview tooling.
- Keep this work in the deployment adapter surface: `scripts/local_k8s.py`,
  `scripts/local_k8s/*`, `Makefile`, deployment documentation, and focused
  tests. Do not alter public API behaviour unless a validation failure proves
  it is necessary.
- Preserve the existing Docker plus `k3d` workflow and the current
  `make local-k8s-up`, `make local-k8s-status`, `make local-k8s-logs`, and
  `make local-k8s-down` targets.
- Keep existing `WILDSIDE_K3D_CLUSTER` and `WILDSIDE_K3D_PORT` overrides as
  backwards-compatible aliases. Add provider-neutral names for new work rather
  than making users set `K3D` variables for `kind`.
- Use `systemd-run --scope --user -p Delegate=yes` when creating a
  Podman-backed `kind` cluster. This matches the working rootless control-group
  pattern from Episodic and avoids requiring privileged Docker.
- For rootless Podman plus `kind`, load images through a temporary archive.
  `kind load docker-image` is for Docker-backed clusters, while
  `kind load image-archive` works with the Podman provider.
- Do not create `kind` host-port mappings for the preview HTTP port. Use
  `kubectl port-forward` for `kind` because host-port mappings reserve the
  same loopback port that the developer-facing preview needs.
- Keep command execution through the existing local preview command abstraction
  unless implementation proves that abstraction cannot express provider
  environment variables or standard input safely.
- Use Makefile targets where they exist. Run checks sequentially and capture
  output with `tee` under `/tmp`.
- Commit only after relevant gates pass.

## Tolerances

- Scope tolerance: if implementation requires replacing the Helm chart,
  changing backend health endpoint paths, or adding Nile Valley repository
  changes in the Wildside branch, stop and ask for approval.
- Compatibility tolerance: if keeping the old `WILDSIDE_K3D_*` variables
  conflicts with clear provider-neutral configuration, stop and record the
  alternatives before changing the public local-preview interface.
- Tooling tolerance: if `kind`, Podman, Helm, `kubectl`, or `systemd-run` are
  absent from the VM, document the missing tool and validate with unit tests,
  Helm rendering, and dry command construction rather than faking a live
  preview.
- Refactor tolerance: `scripts/local_k8s/k3d.py` may be split into
  provider-neutral `cluster.py` and image-loading helpers. If that pushes any
  Python module above 400 lines or creates unclear ownership, stop and split by
  feature.
- Dependency tolerance: do not add new production Rust dependencies. Python
  script dependencies may remain in PEP 723 inline metadata; adding more than
  one new Python dependency requires approval.
- Test tolerance: local-preview behaviour must be covered by pure pytest tests
  that do not require a live cluster. A live Podman plus `kind` smoke test may
  be documented as an optional final validation step.
- Gate tolerance: after three repair attempts on the same gate failure, stop,
  record the command and log path, and ask for direction.

## Risks

Risk: Wildside's current local preview modules are already decomposed around
`k3d`, while Episodic's new helper is provider-neutral from the start.
Mitigation: adapt the behaviour into Wildside's existing module boundaries
first, and only rename or split modules when the `k3d` naming obscures the
new provider contract.

Risk: existing users may have `WILDSIDE_K3D_CLUSTER` or `WILDSIDE_K3D_PORT`
in local shells or scripts. Mitigation: keep those names as aliases and
document the provider-neutral replacements as preferred.

Risk: rootless Podman plus `kind` depends on VM-level cgroup delegation and
the experimental `kind` Podman provider. Mitigation: make the exact command
sequence unit-testable, require `systemd-run`, surface missing tools before
side effects, and document the live smoke command as environment-dependent.

Risk: `kind` cluster inspection uses container-engine JSON that differs from
`k3d cluster list --output json`. Mitigation: parse only the host-port facts
needed for validation and fail with a concise `LocalK8sError` when an existing
cluster cannot be inspected.

Risk: Episodic bootstraps a local Postgres StatefulSet before Helm install,
but Wildside's current `values.local.yaml` does not describe that dependency.
Mitigation: investigate Wildside readiness requirements during implementation;
only add local dependency bootstrap when a failing test or live preview proves
that the existing local values cannot become ready without it.

Risk: the existing `scripts/local_k8s/unittests` pytest suite is not clearly
wired into a Makefile target. Mitigation: run it explicitly during this work
and either wire it into `make test` or document the accepted separate command
in `docs/developers-guide.md` and this plan.

## Current repository orientation

The local preview entry point is `scripts/local_k8s.py`. It uses Cyclopts and
loads `PreviewConfig.from_env()` before delegating to helper modules under
`scripts/local_k8s/`.

The current `PreviewConfig` in `scripts/local_k8s/config.py` carries a
repository root, `k3d` cluster name, Kubernetes namespace, Helm release name,
image name, ingress port, chart path, local values path, and backend
Dockerfile path. The config currently reads `WILDSIDE_K3D_CLUSTER`,
`WILDSIDE_K3D_PORT`, `WILDSIDE_K8S_NAMESPACE`, `WILDSIDE_HELM_RELEASE`, and
`WILDSIDE_IMAGE`.

`scripts/local_k8s/deployment.py` orchestrates the preview. It validates
tools, ensures the cluster, ensures the namespace, builds the Docker image,
imports the image through `k3d`, runs Helm, and prints status.

`scripts/local_k8s/k3d.py` owns cluster lifecycle, image import, and cluster
status. Its name and command construction are the main `k3d` assumptions that
need to become provider-aware.

`scripts/local_k8s/k8s.py` owns namespace creation and Kubernetes status. It
already computes the Helm fullname used for service lookup.

The Wildside chart is under `deploy/charts/wildside`. It already supports
Deployment, Service, ConfigMap, ExternalSecret, ingress, service account,
autoscaling, disruption budget, non-root security context, and health probes.
`deploy/charts/wildside/values.local.yaml` enables local ingress and uses the
local image tag.

The existing design document is `docs/local-k8s-preview-design.md`. It
currently describes a Docker plus `k3d` preview and must be updated to describe
both supported provider modes.

## Episodic prior art

The sibling Episodic commit
`f19cbcf21ad3ecadf164822218ba8e152b3b68bf` added a rootless Podman plus
`kind` path to its Nile Valley preview integration. The transferable decisions
are:

- model the container engine as `docker` or `podman`;
- model the cluster provider as `k3d` or `kind`;
- use `kind-{cluster_name}` and `k3d-{cluster_name}` as kube context names;
- create Podman-backed `kind` clusters with
  `systemd-run --scope --user -p Delegate=yes env
  KIND_EXPERIMENTAL_PROVIDER=podman kind create cluster`;
- give `kind create cluster` a minimal cluster config on standard input and
  avoid `extraPortMappings`;
- build local images with the selected container engine;
- for Podman plus `kind`, remove any stale temporary archive, run
  `podman save --output <archive> <image>`, and then run
  `KIND_EXPERIMENTAL_PROVIDER=podman kind load image-archive <archive>`;
- for Docker plus `kind`, use `kind load docker-image <image>`;
- for `k3d`, keep `k3d image import <image> --cluster <cluster>`;
- inspect existing clusters before reuse and fail clearly when the old
  host-port model conflicts with the desired local preview port;
- print a success banner that names the preview URL, health URL, status,
  logs, teardown, and, for `kind`, the required `kubectl port-forward`
  command.

Episodic also added container-image, Helm-chart, and health-contract tests.
Wildside already has most of those deployment surfaces, so this plan uses
Episodic as a behavioural reference rather than a file-for-file port.

## Implementation plan

Milestone 1 is configuration and tests for provider selection. Add explicit
container-engine and cluster-provider fields to `PreviewConfig`. The preferred
new environment variables are `WILDSIDE_CONTAINER_ENGINE`, with values
`docker` or `podman`, and `WILDSIDE_K8S_PROVIDER`, with values `k3d` or
`kind`. Add `WILDSIDE_K8S_CLUSTER` and `WILDSIDE_K8S_PORT` as
provider-neutral names. Preserve `WILDSIDE_K3D_CLUSTER` and
`WILDSIDE_K3D_PORT` as aliases when the new names are unset. Add validation
tests for accepted values, rejected values, default values, and alias
precedence.

Milestone 2 is provider-aware cluster lifecycle. Introduce provider-neutral
helpers while preserving the public CLI commands. The `k3d` path should keep
the current loopback load-balancer port mapping. The `kind` path should use a
minimal cluster config from standard input, a `kind-{cluster_name}` kube
context, and no HTTP host-port mapping. The Podman-backed `kind` create command
must include `systemd-run --scope --user -p Delegate=yes` and
`KIND_EXPERIMENTAL_PROVIDER=podman`. Tests should record the exact command
sequence for Docker plus `k3d`, Docker plus `kind`, and Podman plus `kind`.

Milestone 3 is provider-aware image build and load. Change image build to call
the selected container engine. Keep `k3d image import` for `k3d`. Add
`kind load docker-image` for Docker-backed `kind`. Add archive save and
`kind load image-archive` for Podman-backed `kind`, including removal of a
stale archive before saving. Tests should assert that Docker is no longer
required when `WILDSIDE_CONTAINER_ENGINE=podman`, and that Podman is required
for both build and archive load in the Podman path.

Milestone 4 is status, logs, and operator output. Make status and logs use the
configured kube context and namespace, so they work after either provider
creates the cluster. For `kind`, print the exact port-forward command:

```bash
kubectl --context kind-wildside-preview --namespace wildside port-forward svc/wildside 8088:80
```

The command should use the Helm-derived service name when the release name and
chart name differ. `down` must be idempotent for both providers and must not
attempt deletion when the provider reports that the cluster is absent.

Milestone 5 is documentation and optional live validation. Update
`docs/local-k8s-preview-design.md` and `docs/developers-guide.md` to describe
the two supported local modes, the provider-neutral environment variables, the
legacy aliases, and the rootless Podman plus `kind` caveats. If local tooling
is available, run a live smoke validation with:

```bash
WILDSIDE_CONTAINER_ENGINE=podman WILDSIDE_K8S_PROVIDER=kind make local-k8s-up
```

Then run the printed `kubectl port-forward` command in a separate terminal and
request `/health/live` from the forwarded port. Tear down with:

```bash
WILDSIDE_CONTAINER_ENGINE=podman WILDSIDE_K8S_PROVIDER=kind make local-k8s-down
```

If the live tools are unavailable, record the missing executable and use the
unit-test and Helm-render gates as the accepted substitute.

## Red, green, refactor sequence

Begin with a red test pass for Milestone 1. Add tests that expect
`PreviewConfig.from_env()` to parse `WILDSIDE_CONTAINER_ENGINE=podman`,
`WILDSIDE_K8S_PROVIDER=kind`, `WILDSIDE_K8S_CLUSTER`, and
`WILDSIDE_K8S_PORT`. Run:

```bash
set -o pipefail
uv run --with pytest --with plumbum --with cyclopts pytest scripts/local_k8s/unittests \
  | tee /tmp/test-local-k8s-wildside-podman-kind-support.out
```

When running this suite directly from the repository root, prefix the command
with `PYTHONPATH=scripts` so the inline script package can be imported:

```bash
set -o pipefail
PYTHONPATH=scripts uv run --with pytest --with plumbum --with cyclopts pytest scripts/local_k8s/unittests \
  | tee /tmp/test-local-k8s-wildside-podman-kind-support.out
```

The observed red result for Milestone 1 was that the new fields or environment
variables did not exist yet.

Make the smallest config change, rerun the focused pytest command, and keep it
green. Then add provider-aware command tests for Milestones 2 through 4 before
changing production helpers. Each new failing test should specify one missing
contract: Podman-backed `kind` creation, Podman archive image loading,
Docker-backed `kind` loading, provider-aware tool preflights, idempotent kind
down, or `kind` port-forward output.

After each green milestone, refactor only enough to keep module ownership
clear and files below 400 lines. Rerun the focused pytest command after every
refactor.

## Validation plan

Run the focused local-preview pytest suite after each milestone:

```bash
set -o pipefail
uv run --with pytest --with plumbum --with cyclopts pytest scripts/local_k8s/unittests \
  | tee /tmp/test-local-k8s-wildside-podman-kind-support.out
```

Use this direct command while the suite is not wired to a Make target:

```bash
set -o pipefail
PYTHONPATH=scripts uv run --with pytest --with plumbum --with cyclopts pytest scripts/local_k8s/unittests \
  | tee /tmp/test-local-k8s-wildside-podman-kind-support.out
```

Run the Markdown gates after documentation changes:

```bash
set -o pipefail
make markdownlint | tee /tmp/markdownlint-wildside-podman-kind-support.out
```

```bash
set -o pipefail
make nixie | tee /tmp/nixie-wildside-podman-kind-support.out
```

Run the repository gates before committing implementation work:

```bash
set -o pipefail
make check-fmt | tee /tmp/check-fmt-wildside-podman-kind-support.out
```

```bash
set -o pipefail
make lint | tee /tmp/lint-wildside-podman-kind-support.out
```

```bash
set -o pipefail
make test | tee /tmp/test-wildside-podman-kind-support.out
```

If live rootless Podman plus `kind` tooling is available, run the optional
smoke path described in Milestone 5 and capture the commands, preview URL, and
teardown result in this plan's `Outcomes & Retrospective`.

## Progress

- [x] 2026-06-22: Loaded the requested `leta`, `rust-router`,
  `python-router`, and `hexagonal-architecture` skills.
- [x] 2026-06-22: Created a leta workspace for the Wildside worktree.
- [x] 2026-06-22: Renamed the local branch from `session/ac25ff3e` to
  `podman-kind-support`.
- [x] 2026-06-22: Inspected Episodic commit
  `f19cbcf21ad3ecadf164822218ba8e152b3b68bf`.
- [x] 2026-06-22: Inspected Wildside's current local preview helper, Helm
  values, existing Nile Valley design document, and local-preview unit tests.
- [x] 2026-06-22: Drafted this pre-implementation ExecPlan.
- [x] 2026-06-22: Ran `make markdownlint`; it passed.
- [x] 2026-06-22: Fixed a pre-existing Mermaid parse error in
  `docs/rstest-bdd-v0-5-0-migration-guide.md` and normalized `bun.lock` so the
  `make nixie` install step is repeatable.
- [x] 2026-06-22: Ran `make --no-print-directory markdownlint nixie`; it
  passed.
- [x] 2026-06-22: User approved this plan for implementation.
- [x] 2026-06-22: Added failing Milestone 1 tests for
  `WILDSIDE_CONTAINER_ENGINE`, `WILDSIDE_K8S_PROVIDER`,
  `WILDSIDE_K8S_CLUSTER`, `WILDSIDE_K8S_PORT`, legacy alias precedence, and
  unsupported provider values.
- [x] 2026-06-22: Implemented Milestone 1 by adding provider fields and
  validation to `PreviewConfig.from_env()`, while keeping Docker plus `k3d` as
  the default.
- [x] 2026-06-22: Ran the focused local preview pytest command with
  `PYTHONPATH=scripts`; it passed with 21 tests.
- [x] 2026-06-22: Ran the Milestone 1 gates after addressing CodeRabbit's
  test-quality findings: focused local preview pytest, `make check-fmt`,
  `make lint`, `make test`, `make markdownlint`, and `make nixie` all passed.
- [x] 2026-06-22: Re-ran `coderabbit review --agent` after the deterministic
  gates. CodeRabbit reported zero findings.
- [x] 2026-06-22: Added failing Milestone 2 tests for provider-aware cluster
  lifecycle command sequences. The first red result was
  `ModuleNotFoundError: local_k8s.cluster`; the follow-up preflight red result
  showed `_deploy_preview_tools()` still assumed the old signature.
- [x] 2026-06-22: Implemented Milestone 2 by introducing
  `local_k8s.cluster`, keeping the k3d loopback load-balancer path, adding
  Docker-backed `kind` cluster creation from stdin, adding the rootless
  Podman-backed `kind` `systemd-run` command, and making deployment preflight
  choose the configured provider tool.
- [x] 2026-06-22: Ran the focused local preview pytest command with
  `PYTHONPATH=scripts`; it passed with 25 tests.
- [x] 2026-06-22: Ran the Milestone 2 gates after addressing CodeRabbit's
  lifecycle and test-quality findings: focused local preview pytest,
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all passed.
- [x] 2026-06-22: Re-ran `coderabbit review --agent` after the deterministic
  Milestone 2 gates. CodeRabbit reported zero findings.
- [x] 2026-06-22: Added failing Milestone 3 tests for provider-aware image
  build and import. The observed red result showed `build_image()` still
  hard-coded Docker and that the Podman-backed kind archive helpers did not
  exist yet.
- [x] 2026-06-22: Implemented Milestone 3 by making image builds use the
  configured container engine, preserving `k3d image import`, preserving
  Docker-backed `kind load docker-image`, and adding Podman-backed
  `podman save` plus `kind load image-archive`.
- [x] 2026-06-22: Ran the Milestone 3 gates: focused local preview pytest,
  `make check-fmt`, `make lint`, and `make test` all passed.
- [x] 2026-06-22: Re-ran `coderabbit review --agent` after the deterministic
  Milestone 3 gates. CodeRabbit reported zero findings.
- [x] 2026-06-22: Added failing Milestone 4 tests for provider-aware status,
  logs, Helm status, kind port-forward output, and idempotent kind teardown.
  The observed red result showed status, logs, and Helm status still omitted
  the selected kube context, and status preflight still required `k3d`.
- [x] 2026-06-22: Implemented Milestone 4 by deriving kube context names from
  the selected provider and cluster, passing that context to namespace, Helm,
  status, and logs commands, and printing the kind port-forward command with
  the Helm-derived service name.
- [x] 2026-06-22: Ran the Milestone 4 gates: focused local preview pytest,
  `make check-fmt`, `make lint`, and `make test` all passed.
- [x] 2026-06-22: Re-ran `coderabbit review --agent` after the deterministic
  Milestone 4 gates. CodeRabbit reported zero findings.
- [x] 2026-06-22: Updated `docs/local-k8s-preview-design.md`,
  `docs/developers-guide.md`, `docs/contents.md`, and the local preview
  package summary, so the documented workflow covers Docker plus `k3d`,
  rootless Podman plus `kind`, provider-neutral environment variables, legacy
  aliases, required tools, kube context naming, and kind port-forward usage.
- [x] 2026-06-22: Ran `make markdownlint` after the Milestone 5 documentation
  edits. The first result found table alignment issues in the new required
  tools table; after aligning the table, `make markdownlint` passed.
- [x] 2026-06-22: Attempted the live rootless Podman plus `kind` smoke path.
  The helper created a Podman-backed `kind` control plane and reached the
  Podman image build, but Helm refused to install because kind's default node
  image used Kubernetes `v1.36.1`, outside the chart's
  `>=1.26.0-0 <1.32.0-0` kubeVersion range.
- [x] 2026-06-22: Added a failing regression test for kind cluster config node
  image pinning, then implemented `WILDSIDE_KIND_NODE_IMAGE` with default
  `kindest/node:v1.31.0` so new kind clusters use a Kubernetes version
  compatible with the Helm chart.
- [x] 2026-06-22: Re-ran the focused local preview pytest command after the
  node-image fix; it passed with 34 tests.
- [x] 2026-06-22: Live Podman plus kind validation then reached Helm rollout,
  but the pod tried to pull `docker.io/library/wildside-backend:local` while
  Podman had exported `localhost/wildside-backend:local`. Added failing tests
  and fixed the Podman archive path by tagging unqualified images with Docker's
  implicit `docker.io/library/...` name before `podman save`.
- [x] 2026-06-22: Live validation next reached container startup and exposed
  that release builds reject ephemeral session keys. Updated local Helm values
  to mount a `sessionSecret`, added helper-generated `wildside-session-key`
  creation before Helm install, and covered the manifest contract in the
  focused deployment tests.
- [x] 2026-06-22: Re-ran the focused local preview pytest command after the
  image-tag and session Secret fixes; it passed with 36 tests. `helm lint
  deploy/charts/wildside --values deploy/charts/wildside/values.local.yaml`
  also passed.
- [x] 2026-06-22: Completed live rootless Podman plus `kind` smoke validation.
  `local-k8s-up` created the Podman-backed kind cluster, built and imported the
  backend image, applied the generated session Secret, installed Helm, and
  reported the deployment as `1/1 Running`. A port-forward to
  `svc/wildside 8088:80` returned
  `{"status":"pass","checks":{"liveness":{"status":"pass"}}}` from
  `/health/live`, and `local-k8s-down` removed the preview cluster.
- [x] Complete Milestone 5 gates and live smoke validation where available.
- [x] 2026-06-22: Ran final deterministic gates for Milestone 5:
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all passed after the live smoke fixes.
- [x] 2026-06-22: Ran `coderabbit review --agent` for Milestone 5 after the
  deterministic gates. CodeRabbit reported zero findings.
- [x] 2026-06-22: Tightened the Podman archive tag normalizer after manual
  pre-commit review so namespaced Docker Hub images such as
  `leynos/wildside-backend:local` are saved as
  `docker.io/leynos/wildside-backend:local`, matching Kubernetes pull
  resolution. Added focused regression coverage.
- [x] 2026-06-22: Re-ran the final gates after the namespaced-image fix.
  Focused local preview tests passed with 37 tests, `make check-fmt`,
  `make lint`, `make test`, `make markdownlint`, and `make nixie` passed, and a
  second `coderabbit review --agent` reported zero findings.

## Surprises & Discoveries

- 2026-06-22: Wildside already has a more decomposed local preview helper than
  Episodic's commit introduced. The adaptation should preserve that structure
  instead of copying Episodic's modules verbatim.
- 2026-06-22: Wildside's Helm chart already includes most Nile Valley-facing
  templates, including ExternalSecret support and health probes. The highest
  value work is local-preview provider support, not chart reconstruction.
- 2026-06-22: The local preview pytest files live under
  `scripts/local_k8s/unittests`, but no Makefile target visibly runs that
  suite. Implementation should address or document that gap.
- 2026-06-22: `make nixie` runs `bun install`, and the checked-in `bun.lock`
  was stale relative to `package.json`. Committing the normalized lockfile
  avoids repeated validation-time drift.
- 2026-06-22: Running `uv run ... pytest scripts/local_k8s/unittests`
  directly does not add `scripts/` to `sys.path`; the direct command needs
  `PYTHONPATH=scripts`. This confirms the earlier risk that the suite is not
  yet wired into a project Make target.
- 2026-06-22: Wildside's existing command adapter used plumbum and had no
  stdin support. `kind create cluster --config -` needs stdin, so Milestone 2
  extended the adapter with an `input_text` parameter rather than adding a
  second runner abstraction.
- 2026-06-22: CodeRabbit suggested moving a `MockCommandResult` import from
  `conftest` to a relative `.conftest` import, but `scripts/local_k8s/unittests`
  is not a Python package. The better fix was to keep `conftest.py` fixture-only
  and define the helper dataclass inside `test_cluster.py`.
- 2026-06-22: The local preview image build was colocated in
  `deployment.py`, while provider-specific image loading belongs with the
  cluster lifecycle adapter. Milestone 3 kept that split: build uses the
  configured container engine, and `cluster.py` owns the provider-specific
  image import commands.
- 2026-06-22: Status and logs were the last visible surfaces that still relied
  on the user's ambient kube context. Using the same provider-derived context
  everywhere makes the preview commands independent of whatever cluster a
  developer last selected with `kubectl config use-context`.
- 2026-06-22: The Helm chart's service name is not always the release name.
  The kind port-forward output therefore needs the same Helm fullname helper
  as status checks; otherwise, renamed releases would print a command for the
  wrong service.
- 2026-06-22: The documentation index, developer guide, and package docstring
  all used k3d-specific language. Milestone 5 needs to update those secondary
  references as well as the main design document; otherwise, repository search
  still advertises the old single-provider contract.
- 2026-06-22: Live Podman plus kind validation revealed that kind's default
  node image can outrun the chart's supported Kubernetes range. The fix is to
  pin a chart-compatible node image by default and expose an override for
  deliberate Kubernetes upgrade testing.
- 2026-06-22: Podman rewrites an unqualified tag such as
  `wildside-backend:local` to `localhost/wildside-backend:local` in the saved
  archive. Kubernetes resolves the same unqualified pod image as
  `docker.io/library/wildside-backend:local`, so the kind node can have the
  image loaded but still fail with `ImagePullBackOff` unless the archive uses
  Docker's implicit registry name.
- 2026-06-22: The same image-name mismatch applies to namespaced Docker Hub
  names such as `leynos/wildside-backend:local`. Kubernetes resolves them as
  `docker.io/leynos/wildside-backend:local`, not as the short Podman-local
  name.
- 2026-06-22: The local backend container is a release build. It refuses
  `SESSION_ALLOW_EPHEMERAL=1`, which is correct for the production safety
  contract. Local preview therefore needs a real generated Kubernetes Secret,
  not an ephemeral-key opt-in.
- 2026-06-26: Audit review found that the provider-aware helper needed firmer
  test boundaries after the first implementation. Orchestration tests now
  assert deploy/status call order, direct cluster-status tests cover the public
  status helper, and property tests cover image-name normalization, Helm
  fullname bounds, and provider-neutral environment alias precedence.
- 2026-06-26: Security review found that environment-derived cluster names and
  kind node image overrides needed validation before reaching filesystem paths
  or YAML. `PreviewConfig` now validates those fields, and kind cluster YAML
  renders the node image as a quoted JSON scalar.
- 2026-06-26: Unit architecture review identified hard-coded randomness and
  temp-path sources. The local session Secret generator and Podman archive
  directory are now injectable for tests while preserving existing defaults for
  CLI callers.

## Decision Log

- 2026-06-22: Use `WILDSIDE_CONTAINER_ENGINE` and `WILDSIDE_K8S_PROVIDER` as
  the preferred new configuration names. Rationale: the old `WILDSIDE_K3D_*`
  names describe one provider and would be misleading for `kind`, but existing
  users still need compatibility aliases.
- 2026-06-22: Do not port Episodic's local Postgres bootstrap by default.
  Rationale: Wildside's current local values and helper do not declare that
  dependency, and adding infrastructure without a failing readiness proof would
  widen the change unnecessarily.
- 2026-06-22: Treat `kind` port access as an operator-started
  `kubectl port-forward` command rather than a background process owned by the
  helper. Rationale: Episodic proved this model for rootless Podman plus
  `kind`, and it avoids lifecycle ambiguity for long-running port-forward
  processes.
- 2026-06-22: Keep provider selection as validated string literals on
  `PreviewConfig` instead of adding a larger abstraction in Milestone 1.
  Rationale: this preserves the current small local-preview module shape and
  gives later milestones a clear dispatch field without widening scope.
- 2026-06-22: Move active lifecycle imports from `local_k8s.k3d` to
  `local_k8s.cluster`, but keep `local_k8s.k3d` as a compatibility re-export.
  Rationale: the CLI should no longer present the provider layer as k3d-only,
  while existing internal imports can keep working during the transition.
- 2026-06-22: Use explicit `match` dispatch for provider-specific lifecycle
  branches in `local_k8s.cluster`. Rationale: `PreviewConfig` validates the
  provider values, and pattern matching keeps unsupported-state handling
  visible at each side effect boundary.
- 2026-06-22: Store the Podman image export in the system temporary directory
  as `{cluster_name}-image.tar` and remove any stale file before saving.
  Rationale: the archive is transient local-preview data and must not live in
  the repository, while using a deterministic path makes retries and tests
  predictable.
- 2026-06-22: Derive kube contexts as `{provider}-{cluster_name}` on
  `PreviewConfig`. Rationale: both `k3d` and `kind` use that naming convention
  for the clusters created by this helper, and keeping the rule in config
  avoids duplicating string construction across namespace, Helm, status, and
  log operations.
- 2026-06-22: Reuse `helm_fullname()` from `local_k8s.k8s` for deployment
  status output. Rationale: service naming is a Helm contract, not a
  kind-specific concern, and the port-forward command must match the service
  that Kubernetes status already inspects.
- 2026-06-22: Default `kind` clusters to `kindest/node:v1.31.0` and allow
  `WILDSIDE_KIND_NODE_IMAGE` overrides. Rationale: the Helm chart currently
  supports Kubernetes `>=1.26.0-0 <1.32.0-0`, while kind's moving default can
  create newer clusters that fail Helm's deterministic kubeVersion check.
- 2026-06-22: For Podman-backed kind imports, retag unqualified image names as
  `docker.io/library/{name}:{tag}` before saving the archive. Rationale:
  Podman's local `localhost/` normalization does not match Kubernetes'
  unqualified-image resolution, and the node image store must contain the name
  in the pod spec.
- 2026-06-22: Treat namespaced Docker Hub image names as unqualified for Podman
  archive export and retag them as `docker.io/{namespace}/{name}:{tag}`.
  Rationale: a slash without an explicit registry is still a short Docker Hub
  reference from Kubernetes' point of view.
- 2026-06-22: Generate and apply `wildside-session-key` during local preview
  setup when the Secret is missing, reuse existing key material on later
  deploys, and mount it through `values.local.yaml` with `SESSION_KEY_FILE`.
  Rationale: release builds must not use ephemeral session keys, local preview
  should remain self-contained and must not commit secret material, and repeat
  deploys should not invalidate active preview sessions.
- 2026-06-26: Keep the injection seams as optional keyword-only parameters on
  the concrete helper functions instead of adding a larger port abstraction.
  Rationale: the preview helper is still a small script boundary, and the
  injected key generator/archive directory are test seams rather than new
  product concepts.

## Outcomes & Retrospective

Implementation is in progress. Milestone 1 establishes provider-neutral
configuration while preserving Docker plus `k3d` defaults and legacy
`WILDSIDE_K3D_*` aliases. Milestone 2 adds provider-aware cluster lifecycle
commands for Docker plus `k3d`, Docker plus `kind`, and rootless Podman plus
`kind`. Milestone 3 adds provider-aware image build and import, including
Podman archive save/load for rootless kind. Milestone 4 makes status, logs,
namespace creation, and Helm status provider-context aware and prints the
operator's kind port-forward command. Milestone 5 documents both local modes
and validates the VM's rootless Podman plus kind path end to end, including
image import, generated session Secret creation, Helm readiness, port-forwarded
health, and teardown. Milestones 1 through 4 passed focused local preview
tests, the full repository gates, relevant Markdown gates where documentation
changed, and CodeRabbit review. Milestone 5 has passed focused tests,
`helm lint`, and live smoke validation; final repository gates and CodeRabbit
review all passed.
