# Remove duplicated ephemeral preview infrastructure

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

No `PLANS.md` file exists in this repository.

## Purpose / Big Picture

Wildside should no longer carry infrastructure and documentation that now live
in the Nile Valley repo. After this change, Wildside retains only application
assets (notably the Helm chart and container images) and its application code,
while infrastructure code, infra-focused scripts, and infra documentation are
removed or replaced with short references to Nile Valley. Success is visible
when the repo contains no duplicated infra modules or infra scripts, the docs
index no longer lists infra guides, and the Wildside Helm chart is confirmed to
match the Nile Valley example chart interface.

## Constraints

- Keep application artefacts: `deploy/charts/wildside`, `deploy/docker`,
  `deploy/nginx`, and application code under `backend/`, `frontend-pwa/`,
  `crates/`, `packages/`, `spec/`, and `deploy/docker-compose.yml` must remain.
- The Wildside Helm chart must continue to conform to the interface used by
  Nile Valley (compare with `../../nile-valley/deploy/charts/example-app`).
- Do not add new dependencies or toolchains. Use existing Makefile targets.
- Documentation must follow `docs/documentation-style-guide.md` and en-GB
  spelling.
- Run `make fmt`, `make markdownlint`, `make nixie`, `make lint`,
  `make check-fmt`, and `make test` before committing.

## Tolerances (Exception Triggers)

- Scope: if removals and updates require touching more than 350 files or a net
  change above 20,000 lines, stop and escalate.
- Interfaces: if Nile Valley requires a chart interface change beyond values
  keys or schema alignment, stop and confirm the expected contract.
- Dependencies: if a new external dependency is needed to complete the work,
  stop and escalate.
- Iterations: if any quality gate fails after two focused fix attempts, stop
  and escalate.
- Ambiguity: if it is unclear whether to keep `deploy/k8s` (HelmRelease and
  Kustomize overlays), stop and request direction before deleting or retaining
  it.

## Risks

    - Risk: Removing infra directories breaks Makefile targets or CI.
      Severity: high
      Likelihood: medium
      Mitigation: update Makefile and `.github/workflows/ci.yml` alongside
      deletions; run all quality gates.

    - Risk: Docs or code still reference deleted infra paths.
      Severity: medium
      Likelihood: high
      Mitigation: run an `rg` audit for `infra/`, `opentofu`, `fluxcd`,
      `ephemeral preview`, and `wildside-infra` references; update or remove.

    - Risk: Helm chart drift from Nile Valley example chart breaks deploy
      automation.
      Severity: medium
      Likelihood: low
      Mitigation: diff `deploy/charts/wildside` against
      `../../nile-valley/deploy/charts/example-app` and align any interface
      differences beyond naming.

## Progress

    - [ ] (2026-01-31 00:00Z) Inventory duplicated infra components and docs.
    - [ ] (2026-01-31 00:00Z) Remove infra code/scripts and update tooling.
    - [ ] (2026-01-31 00:00Z) Remove infra docs and update documentation index.
    - [ ] (2026-01-31 00:00Z) Verify Helm chart interface and container images.
    - [ ] (2026-01-31 00:00Z) Run quality gates and commit changes.

## Surprises & Discoveries

    - Observation: none yet.
      Evidence: none yet.
      Impact: none yet.

## Decision Log

    - Decision: none yet.
      Rationale: none yet.
      Date/Author: to be filled.

## Outcomes & Retrospective

To be completed after implementation.

## Context and Orientation

Wildside currently includes a full copy of the ephemeral preview infrastructure
(now maintained in `../../nile-valley`). The duplicated infrastructure lives in
`infra/`, infra-focused automation and tests under `scripts/` and
`scripts/tests`, and composite actions under `.github/actions/`. Documentation
for OpenTofu, FluxCD, cert-manager, DOKS, and the ephemeral preview platform
lives under `docs/` and is largely mirrored in Nile Valley.

The application artefacts that must remain are the Helm chart at
`deploy/charts/wildside`, the container build assets under `deploy/docker` and
`deploy/nginx`, and the core application code. Nile Valley expects a Helm chart
interface consistent with `deploy/charts/example-app` in the Nile Valley repo;
Wildside's chart should match the same values schema and template inputs.

## Plan of Work

Stage A: Inventory and interface validation (no code changes).

- Compare Wildside infra directories with Nile Valley to confirm duplication
  (`infra/`, `scripts/`, `.github/actions/`, and infra docs). Capture a list of
  directories and files to remove.
- Establish the Helm chart interface contract by comparing
  `deploy/charts/wildside` with
  `../../nile-valley/deploy/charts/example-app` (values schema, values keys,
  and required templates). Record any differences that require action.
- Decide whether `deploy/k8s` is a GitOps artefact that should be removed or
  retained. Escalate if the intent is unclear.

Stage B: Remove duplicated infra code and adjust tooling.

- Remove infra code and helpers that now live in Nile Valley, including
  `infra/`, infra-related scripts under `scripts/`, infra tests under
  `scripts/tests/`, and composite actions under `.github/actions/` that relate
  to infra provisioning.
- Update the Makefile to remove infra-specific targets (`lint-infra`,
  `INFRA_TEST_TARGETS`, `check-test-deps`, and related targets) and adjust
  `make lint` and `make test` accordingly.
- Update `.github/workflows/ci.yml` to remove infra tooling installation and
  infra checks that no longer apply.

Stage C: Documentation removal and alignment.

- Remove infra documentation that is now owned by Nile Valley (OpenTofu module
  designs, ephemeral preview architecture, infra roadmaps, and infra how-to
  guides). Ensure any remaining docs referencing infra are updated to point to
  Nile Valley instead of duplicating content.
- Update `docs/contents.md` to remove the infra section and add a short pointer
  to the Nile Valley repository for preview infrastructure documentation.
- Update `docs/repository-structure.md` to reflect the reduced scope, removing
  infra directories and scripts from the structure listing.
- Scan other docs for infra-specific references (e.g.
  `docs/wildside-backend-architecture.md`) and replace with references to Nile
  Valley if the context still matters.

Stage D: Helm chart and container image confirmation.

- Ensure `deploy/charts/wildside` aligns with the Nile Valley example chart
  interface (values schema, required keys, and templates). If differences are
  only naming (chart name, labels, default secret name), leave them. If value
  keys or schema differ, update the Wildside chart to match the interface.
- Confirm container image build assets remain in `deploy/docker` and
  `deploy/nginx`, and update any chart or docs references to the expected image
  repository/tag interface if needed.

Stage E: Validation and clean-up.

- Run formatting and linting, then tests, capturing output with `tee` to the
  `/tmp/$ACTION-$(get-project)-$(git branch --show).out` naming convention.
- Rerun an audit search for infra references (for example
  `rg -n "infra/|opentofu|fluxcd|ephemeral preview|wildside-infra"`) and
  resolve any remaining unintended hits.
- Commit changes in small, atomic commits once quality gates pass.

## Concrete Steps

1. Inventory duplicate infra content and interface differences.

   - Compare infra trees:
     diff -qr infra ../../nile-valley/infra
     diff -qr scripts ../../nile-valley/scripts
     diff -qr .github/actions ../../nile-valley/.github/actions

   - Compare Helm charts:
     diff -qr ../../nile-valley/deploy/charts/example-app \
       deploy/charts/wildside

   - Audit for infra references:
     rg -n "infra/|opentofu|fluxcd|ephemeral preview|wildside-infra" .

2. Remove infra code and update tooling.

   - Use `git rm -r` on infra directories and infra scripts once the inventory
     confirms duplication.
   - Edit `Makefile` to remove infra targets and `lint-infra` from `make lint`.
   - Edit `.github/workflows/ci.yml` to remove infra tool installs and infra
     checks.

3. Remove infra documentation and update references.

   - Delete infra docs (OpenTofu module designs, ephemeral preview docs, infra
     roadmaps) that are now in Nile Valley.
   - Update `docs/contents.md` and `docs/repository-structure.md` to reflect
     the new scope and point to Nile Valley for infra guidance.
   - Re-run `rg` and update any remaining infra references.

4. Confirm Helm chart interface and container assets.

   - Align `deploy/charts/wildside` values schema and template expectations
     with the Nile Valley example chart if needed.
   - Keep container image build files in `deploy/docker` and `deploy/nginx`.

5. Run quality gates and commit.

   - Run:
     make fmt | tee /tmp/fmt-$(get-project)-$(git branch --show).out
     make markdownlint | tee /tmp/markdownlint-$(get-project)-$(git branch --show).out
     make nixie | tee /tmp/nixie-$(get-project)-$(git branch --show).out
     make lint | tee /tmp/lint-$(get-project)-$(git branch --show).out
     make check-fmt | tee /tmp/check-fmt-$(get-project)-$(git branch --show).out
     make test | tee /tmp/test-$(get-project)-$(git branch --show).out

   - Commit with a descriptive message once all gates pass.

## Validation and Acceptance

Quality criteria (what "done" means):

- `make fmt` and `make check-fmt` pass.
- `make markdownlint` and `make nixie` pass after doc updates.
- `make lint` passes without infra targets.
- `make test` passes and no infra tests remain.
- A final `rg` audit shows no unintended infra references.
- The Helm chart values schema matches the Nile Valley example chart schema
  (differences limited to naming and defaults).

Quality method (how we check):

- Run the Makefile targets listed in Concrete Steps.
- Use `diff -qr` for chart interface comparison.

## Idempotence and Recovery

All removal steps should be repeatable. If a deletion is too broad, restore the
removed paths using git and adjust the removal list before retrying. Re-run the
same Makefile targets after each correction.

## Artifacts and Notes

Record key command outputs (diff summaries and audit searches) in the commit
messages or attach the `tee` logs for reference. Keep only the log paths in the
repo notes, not the full logs.

## Interfaces and Dependencies

The Wildside Helm chart must expose the same values interface as the Nile
Valley example chart at `../../nile-valley/deploy/charts/example-app`:

- `image.repository`, `image.tag`, `image.pullPolicy`
- `config` for non-secret environment variables
- `existingSecretName`, `allowMissingSecret`, `secretEnvFromKeys`
- `sessionSecret` with `enabled`, `name`, `keyName`, and `mountPath`
- `ingress` values (`enabled`, `className`, `hostname`, `tlsSecretName`)
- standard `service`, `resources`, `autoscaling`, and security contexts

Container images must remain buildable from `deploy/docker/*.Dockerfile` and
configured in chart values via `image.repository` and `image.tag` so Nile Valley
can inject preview tags.
