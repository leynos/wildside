# Adapt Wildside local previews for rootless Podman and kind

This ExecPlan is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT. This plan is ready for review but has not been approved for
implementation.

## Purpose / big picture

Wildside already exposes the Nile Valley-facing deployment contract through the
backend container image, the Helm chart under `deploy/charts/wildside`, health
probes, and the repository-local `scripts/local_k8s.py` preview helper. That
preview helper currently assumes Docker plus `k3d`. The Episodic commit
`f19cbcf21ad3ecadf164822218ba8e152b3b68bf` shows how a sibling service was
adapted so local previews can run on this virtual machine with rootless Podman
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

- Do not implement this plan until the user explicitly approves it.
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
  `kubectl port-forward` for `kind`, because host-port mappings reserve the
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

The expected red result is that the new fields or environment variables do not
exist yet.

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
- [ ] Review and approve this plan.
- [ ] Implement Milestone 1.
- [ ] Implement Milestone 2.
- [ ] Implement Milestone 3.
- [ ] Implement Milestone 4.
- [ ] Implement Milestone 5.
- [ ] Run final gates and live smoke validation where available.

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

## Outcomes & Retrospective

This plan is pre-implementation. No production code has been changed yet. The
expected implementation outcome is a backwards-compatible local preview helper
that supports Docker plus `k3d`, Docker plus `kind`, and rootless Podman plus
`kind`, with focused tests documenting each provider-specific command contract.
