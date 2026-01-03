# Design the wildside-infra GitOps tree automation

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

No PLANS.md file was found in the repository root when preparing this plan.

## Purpose / Big Picture

Update `docs/cloud-native-ephemeral-previews.md` so it contains a concrete,
idempotent automation design for laying out the `wildside-infra` GitOps tree.
Success is observable when the document describes the automated layout flow,
names the tool choice (Python vs OpenTofu) with rationale, lists the required
`platform` subdirectories (including CloudNativePG and Redis), and the
documentation checks pass (`make fmt`, `make markdownlint`, `make nixie`,
`make check-fmt`).

## Progress

    - [x] (2026-01-03 03:46Z) Drafted this ExecPlan for review.
    - [x] (2026-01-03 03:59Z) Reviewed roadmap, design docs, and module outputs
      to confirm the target GitOps tree and gaps.
    - [x] (2026-01-03 03:59Z) Revised
      `docs/cloud-native-ephemeral-previews.md` with the automation design,
      updated tree, and tool selection rationale.
    - [x] (2026-01-03 12:56Z) Ran documentation formatting and lint checks
      (`make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`).
    - [x] (2026-01-03 13:20Z) Repaired Table 1 formatting in
      `docs/cloud-native-ephemeral-previews.md` and restored reference link
      definitions in `docs/contents.md`.

## Surprises & Discoveries

    - Observation: Module outputs map CloudNativePG to `platform/databases` and
      Valkey (Redis) to `platform/redis`, so the design must align with those
      paths.
      Evidence: `infra/modules/cnpg/manifests.tf` and
      `infra/modules/valkey/manifests.tf`.
    - Observation: Markdownlint failures were driven by Table 1 formatting and
      missing reference definitions, resolved by restoring aligned table
      spacing and defining the missing references.
      Evidence: `/tmp/make-markdownlint.log`.
    - Observation: `make nixie` required escalated permissions for Mermaid
      diagram rendering to avoid Chromium sandbox restrictions.
      Evidence: `/tmp/make-nixie.log`.

## Decision Log

    - Decision: Prefer Python scripting in the `wildside-infra-k8s` workflow to
      materialize the GitOps tree; keep OpenTofu for infrastructure and manifest
      rendering.
      Rationale: OpenTofu is excellent for resource lifecycle management but is
      not well suited to deterministic file system tree generation, whereas a
      Python helper can safely render `rendered_manifests` outputs, create the
      directory layout, and manage drift with precise, idempotent writes inside
      CI.
      Date/Author: 2026-01-03 / Codex
    - Decision: Align the design with existing module output paths
      (`platform/databases` for CloudNativePG and `platform/redis` for
      Redis-compatible Valkey).
      Rationale: The OpenTofu modules already emit manifests keyed to these
      paths, so the GitOps layout should match to avoid extra translation
      layers.
      Date/Author: 2026-01-03 / Codex

## Outcomes & Retrospective

Updated the design document with an idempotent GitOps tree automation plan and
directory layout updates. Documentation validation now passes after fixing
table formatting and rerunning the lint and Mermaid checks.

## Context and Orientation

The roadmap item in `docs/ephemeral-previews-roadmap.md` requires the
`wildside-infra` GitOps repository to contain `clusters/<cluster>/`,
`modules/`, and a `platform/` tree with subdirectories for `sources`,
`traefik`, `cert-manager`, `external-dns`, `vault`, and shared data services
(CloudNativePG and Redis). The design document at
`docs/cloud-native-ephemeral-previews.md` already mentions the target layout
and the `wildside-infra-k8s` action but does not yet describe an end-to-end,
idempotent automation for generating the tree.

The OpenTofu interoperability contract in
`docs/opentofu-module-interoperability-contract.md` defines a
`rendered_manifests` output map keyed by GitOps paths. The modules live in
`infra/modules/` and are named `cnpg` (CloudNativePG) and `valkey`
(Redis-compatible); their outputs already map to `platform/databases` and
`platform/redis`, and the design update should reflect those paths. The
repository guidelines in `docs/scripting-standards.md` govern any Python helper
referenced by the design.

## Plan of Work

First, review the existing GitOps layout description in
`docs/cloud-native-ephemeral-previews.md`, the roadmap item, and the current
module output paths to confirm the canonical directory names for data services.
The module outputs already use `platform/databases` for CloudNativePG and
`platform/redis` for Redis-compatible Valkey, so the design should align with
those paths and describe them explicitly in the updated layout.

Next, update the design document to add a focused subsection describing the
automated, idempotent tree materialization performed by the
`wildside-infra-k8s` action. The section should explain that a Python helper
drives OpenTofu in render mode, reads the `rendered_manifests` output map, and
writes deterministic files into `clusters/<cluster>/`, `modules/`, and the
`platform/` subtree. Explicitly state that the platform tree is fully managed
by automation, so manual edits are overwritten on each run.

Then, revise the repository structure table and any path examples in the design
document to match the chosen layout, ensuring the `platform` subdirectories are
listed explicitly (including data services). Add a short rationale section
explaining why Python is the correct tool for file system generation and drift
reconciliation, while OpenTofu remains the engine for infrastructure state and
manifest rendering.

Finally, format and lint the documentation and confirm there are no lingering
references to the old paths.

## Concrete Steps

1. Review the current descriptions and module outputs.

       rg -n "platform/|wildside-infra-k8s|rendered_manifests" \
         docs/cloud-native-ephemeral-previews.md \
         docs/opentofu-module-interoperability-contract.md
       rg -n "output \"rendered_manifests\"" -S infra/modules

   Expected: matches for the existing layout table, platform examples, and
   module output keys.

2. Edit `docs/cloud-native-ephemeral-previews.md` with the new automation
   section, updated tree layout, and tool selection rationale.

       apply_patch
       *** Begin Patch
       …
       *** End Patch

   Expected: the file includes a new automation subsection, updated paths, and
   tool choice rationale.

3. Run documentation formatting and lint checks (300 second timeout, capture
   logs).

       set -o pipefail
       timeout 300 make fmt 2>&1 | tee /tmp/make-fmt.log
       timeout 300 make markdownlint 2>&1 | tee /tmp/make-markdownlint.log
       timeout 300 make nixie 2>&1 | tee /tmp/make-nixie.log
       timeout 300 make check-fmt 2>&1 | tee /tmp/make-check-fmt.log

   Expected: each command exits 0 with no errors; logs are empty of failures.

## Validation and Acceptance

- The design document at `docs/cloud-native-ephemeral-previews.md` contains a
  new subsection that describes the idempotent GitOps tree automation and
  explicitly states the tool choice with rationale.
- The GitOps tree layout in the design document includes `clusters/<cluster>/`,
  `modules/`, `platform/sources/`, `platform/traefik/`,
  `platform/cert-manager/`, `platform/external-dns/`, `platform/vault/`, and
  data service directories matching the module outputs (CloudNativePG and
  Redis-compatible).
- `rg -n "platform/(cnpg|valkey)" docs/cloud-native-ephemeral-previews.md`
  returns no references to superseded paths, if they were replaced.
- `make fmt`, `make markdownlint`, `make nixie`, and `make check-fmt` all pass.

## Idempotence and Recovery

Documentation edits are safe to repeat. If a formatting or lint step fails,
re-run the command after fixing the reported lines. If the design update needs
to be retried, re-apply the patch and re-run the formatting commands; the
results should converge without manual clean-up.

## Artifacts and Notes

Example of the intended GitOps tree (paths may adjust to match module outputs):

    clusters/<cluster>/
    modules/
    platform/
      sources/
      traefik/
      cert-manager/
      external-dns/
      vault/
      databases/   # CloudNativePG
      redis/       # Redis-compatible

## Interfaces and Dependencies

The design update should describe a Python helper invoked by the
`wildside-infra-k8s` action that materializes the GitOps tree. Document the
helper as exposing a function with a stable signature, for example:

    render_gitops_tree(output_dir: Path, rendered: dict[str, str],
                       cluster: str) -> RenderReport

The helper should read OpenTofu `rendered_manifests` outputs and write files to
the target paths deterministically, using only the Python standard library
unless a dependency is explicitly justified.

## Revision note (required when editing an ExecPlan)

Updated progress and surprises to capture the Table 1 fix and restored
reference definitions, keeping validation status current.
