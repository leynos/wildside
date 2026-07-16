# Catalogue contradictions and duplicated requirements across front-end sources

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item 0.1.2 produces a catalogue of contradictions and duplicated
requirements across the Wildside front-end source documents and contracts.
Roadmap item 0.1.1 has already named the authoritative source for each topic;
0.1.2 now goes one level deeper, enumerating concrete clashes between specific
paragraphs, schemas, or operations and labelling each finding with a follow-up
category from a fixed set. After this work is complete, a contributor opening
the catalogue can see, for any phase 1-5 implementation task, whether the cited
sources agree, where they disagree, and which downstream design-document or
Architecture Decision Record (ADR) update must land before the task can cite a
coherent authority.

The deliverable is a Markdown catalogue at
`docs/frontend-source-contradictions-catalogue.md`. It is documentation-only.
It introduces no runtime behaviour, runtime strings, card model fixtures,
Application Programming Interface (API) contracts, dependencies, lint rules,
test harnesses, or generated artefacts. A small auditing script under
`scripts/` is the only executable artefact, and it serves the catalogue's
reproducibility rather than the application's runtime.

Six months on, success looks like this: every phase 1-4 implementation task
in `docs/frontend-roadmap.md` can be reviewed by opening the catalogue,
finding the affected topic, and confirming whether the task's cited
authorities currently agree. The catalogue stays load-bearing because closing
PRs update the relevant rows rather than leaving stale `open` entries.

Implementation proceeded after approval and is complete.

## Constraints

The implementation must satisfy `docs/frontend-roadmap.md` item 0.1.2. It must
not resolve any of the findings it identifies. Resolution lives in the owning
design document, the data model, the ADR set, the OpenAPI or AsyncAPI specs,
or in a targeted roadmap citation fix. Adding requirement prose to
`docs/frontend-roadmap.md` to settle a finding is an explicit failure of the
roadmap's success criterion for this item ("no finding is resolved by adding
requirement prose only to this roadmap"), and the catalogue must call that
prohibition out for reviewers.

Every finding must use exactly one label from the prescribed set: `update
design document`, `merge into Progressive Web App design`, `update data
model`, `write Architecture Decision Record`, `roadmap citation fix`. Adding
new labels is out of scope and requires an explicit decision in the Decision
Log.

Contract gaps in `spec/openapi.json` and `spec/asyncapi.yaml` are mapped onto
the prescribed labels using the following ownership decision tree. The
catalogue applies the tree to every contract-gap finding and records the
chosen branch in the finding's rationale.

1. The finding is about a missing or wrong **field** inside an existing
   endpoint, request body, response body, channel payload, or event shape.
   Label: `update data model`. The data model owns entity shape; the contract
   change is a downstream follow-up that the data-model amendment dictates.
2. The finding is about a missing **endpoint surface** that prose or the user
   experience state graph declares but `spec/openapi.json` omits, or about a
   missing **channel/event surface** that prose declares but
   `spec/asyncapi.yaml` omits. Label: `update design document`. The Wildside
   PWA design owns endpoint and channel surface intent; the contract change
   follows. Sub-resolution: `update OpenAPI` or `update AsyncAPI`.
3. The finding is about a wire-level encoding detail (header naming, status
   code semantics, error payload shape) where prose and design already agree
   on intent. Label: `update design document`. Sub-resolution: `update
   OpenAPI` or `update AsyncAPI`.
4. The finding is about a cross-cutting platform invariant (idempotency
   key TTL, retry semantics, conflict-resolution policy, service-worker
   update strategy) that prose, contract, and runtime would all need to
   honour. Label: `write Architecture Decision Record`. The ADR settles the
   invariant; design, data model, and contract follow.

Worked example: F-001 (image alt text shape — plain string in the data model,
localized map in the v2a stack reference) takes branch 1 and is labelled
`update data model`. F-009 (catalogue, route generation, and offline bundle
endpoint families absent from `spec/openapi.json`) takes branch 2 and is
labelled `update design document` with sub-resolution `update OpenAPI`.
F-015 (idempotency-key contract scope) takes branch 4 and is labelled `write
Architecture Decision Record`.

`docs/v2a-front-end-stack.md` takes precedence where it conflicts with older
Progressive Web Application (PWA) platform guidance. The catalogue must not
silently merge incompatible guidance and must not promote any one document to
authority on a topic that the source authority catalogue assigns elsewhere.

The roadmap remains an implementation queue, not a design authority. Product
policy, schema shape, platform policy, and user experience rules must live in
design documents, ADRs, `spec/openapi.json`, `spec/asyncapi.yaml`, or
`docs/wildside-ux-state-graph-v0.1.json`. The roadmap may cite those sources
and track work.

Hexagonal architecture boundaries must be preserved when describing findings.
Findings that touch domain-versus-adapter ownership (for example, whether the
backend ships presentation classes through entity schemas, or whether
WebSocket events are an inbound adapter concern) must be phrased as ownership
clarifications, not as new front-end coupling to backend internals. API
contracts are consumed through OpenAPI, AsyncAPI, generated client boundaries,
or documented ports. The catalogue itself is documentation; the
`hexagonal-architecture` skill applies only to the way the catalogue describes
findings about hexagonal boundaries.

Every finding entry must cite both clashing sources by full repository-relative
path and either an anchor or a line range. Quoted fragments are limited to
short excerpts (no more than roughly 25 words per claim) so the catalogue
remains a navigational aid, not a redistribution of the source documents.

The closing PR for any finding must update the affected row's `status` field
to `resolved by PR #NNNN` (or `superseded` if the finding is no longer
applicable). This obligation is recorded in the catalogue's introduction and
pointed at from `docs/developers-guide.md` when the latter is amended, so the
catalogue stays load-bearing rather than drifting to "everything is open".

The implementation must use en-GB-oxendict spelling and the Markdown rules in
`docs/documentation-style-guide.md`. Paragraphs and bullets wrap at 80 columns,
fenced code blocks declare a language (including `plaintext` for text), and
any Mermaid diagrams include accessible prose where added.

Translatable content is not introduced by this catalogue. If any change would
require a new runtime string or card model label, implementation stops and
escalates rather than adding strings without per-locale and right-to-left
(RTL) coverage per `docs/v2a-front-end-stack.md`.

Wildside targets Web Content Accessibility Guidelines (WCAG) 2.2 Level AA for
front-end work. For this documentation-only catalogue, the acceptance
criterion is that the catalogue names the authoritative accessibility source
and the future validation gates. Any executable front-end change discovered
during implementation must pass browser-level validation with no errors,
failures, or accessibility violations before commit.

Run validation commands sequentially and capture long output with `tee` under
`/tmp` using `/tmp/$ACTION-$(get-project)-$(git branch --show-current).out` as
the filename template. Do not run format, lint, or test commands in parallel.
Prefer Makefile targets over package-local commands.

Commit only after the relevant gates pass. Use a file-based commit message via
`git commit -F`; do not use `git commit -m`.

## Tolerances (exception triggers)

Escalate before continuing if implementing this plan requires changing more
than three source documents other than the new catalogue, the new audit
script under `scripts/`, the roadmap status update, this ExecPlan, and
optionally `docs/developers-guide.md`. The expected steady-state edit set is:
one new catalogue document, one small state-graph walker under `scripts/`,
one roadmap checkbox update on completion, this ExecPlan, and possibly a
small developers-guide update describing where to record contradictions in
future. Editing any of the eight primary source documents to *settle* a
finding is out of scope for 0.1.2; the catalogue records the follow-up but
does not perform it.

Escalate if any backend Rust source, TypeScript source, package manifest,
lockfile, `spec/openapi.json`, `spec/asyncapi.yaml`, generated artefact, or
migration must change. Item 0.1.2 is about contradiction triage, not contract
or runtime implementation. Note that finding *that* a contract needs change is
expected and recorded; *making* the change is downstream.

Escalate if a finding cannot be labelled with one of the five prescribed
labels without distortion. Capture the awkward case in the Decision Log and
present options before introducing a new label.

Escalate if the catalogue cannot be drafted without exceeding a single
document. Splitting into multiple files is a structural decision that requires
sign-off because the source authority catalogue is a single file and the two
documents are designed to be read together.

Escalate if Playwright or `css-view` validation is impossible for a reason
other than "no executable front-end surface changed". If no executable surface
changed, record that fact in the catalogue evidence section and still run the
available documentation and repository gates.

Escalate if `make check-fmt`, `make lint`, or `make test` fails twice after
clear, relevant fixes. Do not work around failures by narrowing the gate or
silencing lints. The same applies to `make markdownlint` and `make nixie`.

Escalate if `coderabbit review --agent` reports a concern that cannot be
resolved without expanding the scope beyond this catalogue.

When Stage B.3 triage shows surviving findings approaching 40, apply the
umbrella-finding rule before continuing rather than waiting to trip the
tolerance: collapse cohesive sibling findings (for example, several
identifier-level variants of the same schema-shape clash) into one umbrella
finding with a `siblings:` list in the row schema. This preserves coverage
while keeping the catalogue reviewable. Escalate only if umbrella collapse
still leaves more than 40 surviving findings; that volume signals a survey
method that is too permissive.

## Risks

Risk: the catalogue becomes a hidden design document.
Severity: high.
Likelihood: medium.
Mitigation: every finding entry must point at an owning document and a
follow-up label; the catalogue itself does not contain new product policy.
If a finding looks like it needs policy prose to be intelligible, name an ADR
or design-document follow-up and link to the owning source for context.

Risk: drift between this catalogue and the source authority catalogue.
Severity: medium.
Likelihood: medium.
Mitigation: every finding entry cites the relevant topic row in
`docs/frontend-source-authority-catalogue.md`. The two documents reference
each other in their introductions, and an explicit topic cross-reference table
appears in this catalogue.

Risk: over-counting contradictions.
Severity: medium.
Likelihood: high.
Mitigation: discard candidates where two sources merely repeat the same
requirement consistently or use synonyms with the same meaning. Each finding
must be a real reconcilable gap. Apply the BCP 14 keyword test (see Decision
Log): if both sources can be rewritten to identical MUST/SHOULD/MAY statements
without losing meaning, the candidate is duplication, not contradiction. If
they cannot, it is a contradiction.

Risk: stack version drift between `frontend-pwa/` (Tailwind v3, DaisyUI v4)
and the v2a target (Tailwind v4, DaisyUI v5) generates many noise findings.
Severity: medium.
Likelihood: high.
Mitigation: the source authority catalogue already classifies this as an
`implementation follow-up` for 0.2.4 and 0.2.5; the contradictions catalogue
records the stack drift as a single confirmed `roadmap citation fix` and does
not duplicate it per topic. Per-topic variants are appended only when they
encode a substantively different requirement (for example, the Tailwind v4
`@plugin` and `@utility` syntax for semantic registration).

Risk: machine-readable contracts (OpenAPI, AsyncAPI, UX state graph) are
audited differently from prose docs and miss real gaps.
Severity: medium.
Likelihood: medium.
Mitigation: a small grep-driven cross-reference step extracts every
`operationId`, channel name, and state identifier and asserts the design
documents mention them, before structured prose review. This catches missing
endpoints and orphan states that prose review alone would skip.

Risk: the audit window biases toward known hotspots in
`docs/frontend-source-authority-catalogue.md` and misses unflagged areas.
Severity: medium.
Likelihood: medium.
Mitigation: the structured audit pass covers each of the eight primary sources
end-to-end and records audited section ranges, not only the hotspot list.

Risk: hexagonal boundary findings are misclassified as design issues when they
are really domain-versus-adapter contracts.
Severity: low.
Likelihood: medium.
Mitigation: the audit applies the `hexagonal-architecture` skill's invariants
(dependency rule, port ownership, domain purity, adapter isolation) to each
finding that touches the domain-adapter boundary, and surfaces those findings
with explicit ownership rationale.

Risk: catalogue grows stale as design docs are amended downstream.
Severity: medium.
Likelihood: high (over months).
Mitigation: each finding row carries a status field (`open`, `resolved by
PR #NNNN`, `superseded`) so the catalogue is updated as findings close
rather than rewritten. The Decision Log captures the convention.

## Progress

Use a list with checkboxes to summarize granular steps. Every stopping point
must be documented here, even if it requires splitting a partially completed
task into two ("done" vs. "remaining"). This section must always reflect the
actual current state of the work.

- [ ] Stage A.1 — Lock the label set, the finding-identifier scheme, and the
  catalogue file path against the ExecPlan; record any deviations in the
  Decision Log before any audit work begins.
- [ ] Stage A.2 — Confirm the eight primary sources, the supporting cross-
  reference set, and the hotspot priors from
  `docs/frontend-source-authority-catalogue.md` are still current at audit
  time. Record any new sources discovered.
- [ ] Stage A.3 — Establish the grep-driven cross-reference script in scratch
  form (not committed) for OpenAPI operations, AsyncAPI channels, and UX
  state-graph identifiers.
- [ ] Stage B.1 — Run the grep-driven cross-reference pass; capture the
  resulting orphan/missing identifier list as a candidate-findings draft in
  the ExecPlan's `Surprises & Discoveries`.
- [ ] Stage B.2 — Run a structured prose audit of the eight primary sources
  against the hotspot priors and the cross-reference output; capture each
  candidate finding with file:line citations, BCP 14 keyword annotations
  where useful, and a proposed label.
- [ ] Stage B.3 — Triage the candidate-findings draft: collapse duplicates,
  drop stylistic variances, confirm catalogue-recorded reconciliations rather
  than re-analysing them, and decide severity for each surviving finding.
- [ ] Stage C.1 — Author
  `docs/frontend-source-contradictions-catalogue.md`. Mirror the introductory
  framing of `docs/frontend-source-authority-catalogue.md`; replace topic
  classifications with finding rows.
- [ ] Stage C.2 — Add the catalogue's findings index, topic cross-reference
  table, validation note, and relevant-skills section.
- [ ] Stage C.3 — Update `docs/developers-guide.md` only if reviewers need a
  pointer to the new catalogue. Keep changes minimal.
- [ ] Stage D.1 — Run `make check-fmt`, `make lint`, `make markdownlint`, and
  `make nixie` sequentially with `tee` capture. Resolve any failures.
- [ ] Stage D.2 — Run `make test` and document its result. No executable
  front-end surface is expected to change.
- [ ] Stage D.3 — Run `coderabbit review --agent`; resolve concerns before
  proceeding to the next milestone.
- [ ] Stage D.4 — Mark roadmap item 0.1.2 as done in
  `docs/frontend-roadmap.md`. Commit and push.
- [ ] Stage D.5 — Update PR description with completion notes, summarize
  outcomes here, and move the ExecPlan status to COMPLETE.

Progress notes:

- [x] Stage A.1 — Locked the existing five-label set, `FIND-NNNN`
  identifier scheme, canonical row schema, `status` and `perishability`
  fields, and catalogue path
  `docs/frontend-source-contradictions-catalogue.md`.
  `(2026-06-14 22:51Z)`
- [x] Stage A.2 — Confirmed the audit source set is present: the authority
  catalogue, v2a stack, accessible PWA guide, semantic Tailwind guide,
  Wildside PWA design, Wildside PWA data model, UX state graph, sitemap,
  OpenAPI spec, and AsyncAPI spec. Supporting hotspot documents remain the
  same set named in the ExecPlan. `(2026-06-14 22:51Z)`
- [x] Stage A.3 — Added the reproducible UX state-graph walker at
  `scripts/audit-ux-state-graph.mjs`; OpenAPI and AsyncAPI extraction remain
  scratch commands under `/tmp` as planned. `(2026-06-14 22:52Z)`
- [x] Stage B.1 — Ran the identifier inventory into `/tmp` logs using the
  `audit-$(get-project)-$(git branch --show-current)` template. The pass found
  11 OpenAPI operations, one AsyncAPI channel, 74 UX states, and 18 UX state
  orphan markers for triage. `(2026-06-14 22:52Z)`
- [x] Stage B.2 — Completed the structured prose audit against source-authority
  hotspots, contract inventories, and the wyvern confirmation pass.
  Candidate findings were captured for schema shape, presentation leakage,
  REST and AsyncAPI gaps, idempotency, auth phase boundaries,
  service-worker update policy, stack drift, and navigation terminology.
  `(2026-06-14 22:56Z)`
- [x] Stage B.3 — Triaged candidates down to 13 findings and two
  duplicate-but-consistent rows. Dropped or retained UX orphan markers only
  where cross-document evidence supported a real finding. `(2026-06-14
  22:56Z)`
- [x] Stage C.1 — Authored
  `docs/frontend-source-contradictions-catalogue.md` with scope, label set,
  contract-gap ownership tree, status convention, row-update obligation, and
  canonical row schema. `(2026-06-14 22:57Z)`
- [x] Stage C.2 — Added findings, duplicate triage rows, topic
  cross-reference table, coverage matrix, audit artefacts, validation note,
  and relevant-skills section. `(2026-06-14 22:57Z)`
- [x] Stage C.3 — Updated `docs/developers-guide.md` with a pointer to the
  catalogue and the row-status update obligation for closing pull requests.
  `(2026-06-14 22:58Z)`
- [x] Stage D.1 — Ran `make check-fmt`, `make lint`, `make markdownlint`,
  and `make nixie` sequentially with `/tmp` `tee` logs. `markdownlint`
  initially failed on table alignment in the new catalogue and passed after a
  mechanical table-alignment pass. `nixie` initially failed on an existing
  Mermaid label in `docs/rstest-bdd-v0-5-0-migration-guide.md` and passed
  after quoting the multi-line labels. `(2026-06-15 00:11Z)`
- [x] Stage D.2 — Full `make test` rerun passed after the earlier transient
  embedded-PostgreSQL bootstrap failure. Final gate results were 1,286/1,286
  Rust tests passed with four skipped, 47 root Vitest tests passed, 43
  `frontend-pwa` Vitest tests passed, and token contrast checks passed.
  `(2026-06-15 00:17Z)`
- [x] Stage D.3 — Ran `coderabbit review --agent`. The first review reported
  one minor grammar concern in the FIND-0010 rationale; that comma fix was
  applied, `make markdownlint`, `make check-fmt`, and `make lint` were rerun,
  and the follow-up CodeRabbit review completed with zero findings.
  `(2026-06-14 23:59Z)`
- [x] Stage D.4 — Marked roadmap item 0.1.2 complete in
  `docs/frontend-roadmap.md` and linked it to
  `docs/frontend-source-contradictions-catalogue.md`.
  `(2026-06-14 23:59Z)`
- [x] Stage D.5 — Updated PR #375 with implementation completion notes,
  validation results, and the clean follow-up CodeRabbit review status.
  Moved this ExecPlan to COMPLETE. `(2026-06-15 00:00Z)`
- [x] Post-completion hook reconciliation — Committed `0f25910` to
  normalize `bun.lock` after repeated stop-hook checks found the Bun-resolved
  lockfile dirty again after restoration. `make check-fmt`, `make lint`, and
  `make test` passed before the follow-up commit, and the branch was pushed.
  `(2026-06-15 23:06Z)`
- [x] Post-completion audit maintenance — Refreshed the front-end package
  graph to clear pnpm audit advisories for Vite, esbuild, ws, js-yaml,
  `@babel/core`, DOMPurify, and markdown-it without adding audit exceptions.
  `make audit` then exposed new RustSec vulnerabilities in the
  `tokio-postgres`/`postgres-protocol` path, so `Cargo.lock` was updated to
  `tokio-postgres` `0.7.18` and `postgres-protocol` `0.6.12` under existing
  manifest constraints. `(2026-06-17 09:52Z)`
- [x] Post-completion validation — Ran the required format, lint, test,
  documentation, and audit gates after the dependency refresh. Attempted
  `coderabbit review --agent` twice after the deterministic gates passed; both
  invocations reached sandbox preparation and then stopped emitting progress,
  so the stuck review processes were terminated without a review result.
  `(2026-06-17 10:08Z)`

Use ISO 8601 timestamps in UTC (for example, `(2026-06-05 12:34Z)`) when
ticking items to measure rates of progress and detect tolerance breaches.

## Surprises & discoveries

This section records unexpected findings encountered during implementation.
At plan-draft time, the only seeded entries are the candidate findings
produced by the planning research pass; they are not authoritative until the
Stage B audit confirms each one against the actual source text. Each Stage B
audit step appends evidence in this section.

- Observation: planning research surfaced roughly 28 candidate findings across
  schema shape, persistence, routing intent, state-machine semantics, and
  contract gaps. The largest concentration is in the data-model versus
  card-architecture pairing and in the OpenAPI/AsyncAPI gap pair.
  Evidence: research dispatch report from this branch's planning session.
  Impact: Stage B.3 must triage and confirm each candidate before it enters
  the catalogue; do not promote candidates verbatim.

- Observation: implementation began on branch
  `frontend-0-1-2-catalogue-contradictions-and-duplicated-requirements`, not
  on the main branch. The expected source set is present at audit time.
  Evidence: `git branch --show-current`; `wc -l` over
  `docs/frontend-source-authority-catalogue.md`, `docs/v2a-front-end-stack.md`,
  `docs/building-accessible-and-responsive-progressive-web-applications.md`,
  `docs/semantic-tailwind-with-daisyui-best-practice.md`,
  `docs/wildside-pwa-design.md`, `docs/wildside-pwa-data-model.md`,
  `docs/wildside-ux-state-graph-v0.1.json`, `docs/sitemap.md`,
  `spec/openapi.json`, and `spec/asyncapi.yaml`.
  Impact: Stage A can proceed without a branch change or source-set
  escalation.

- Observation: a wyvern agent team member was dispatched to confirm Stage A/B
  inputs and candidate findings without editing files.
  Evidence: agent `019ec854-d8ae-73b3-bdd7-aa8fd63314ef` (`Leibniz`).
  Impact: findings entering the catalogue will be cross-checked against an
  independent read-only pass.

- Observation: `yq` is not installed in this worktree environment, but Python
  with PyYAML is available and can read `spec/asyncapi.yaml` without changing
  project dependencies.
  Evidence: `command -v yq` returned no path; the Stage B.1 Python/YAML
  extraction emitted the `/ws` channel.
  Impact: AsyncAPI extraction remains scratch-only and no new tooling is added
  to the repository.

- Observation: the Stage B.1 UX graph pass emitted 18 orphan markers:
  `router.not_found`, `runtime.service_worker_update_available`,
  `explore.stale_catalogue`, `customize.generate_planned`,
  `route_generation.draft`, `route_generation.conflict`,
  `route_generation.data_sparse`, `route_generation.cancelled`, `map.layout`,
  `map.location_denied`, `map.canvas_error`, `saved.empty`,
  `offline.bundle_complete`, `offline.storage_pressure`, `auth.unknown`,
  `auth.guest`, `auth.authenticated`, and `auth.login`.
  Evidence: the `.out.uxstates` log written under `/tmp` with the
  `audit-$(get-project)-$(git branch --show-current)` template.
  Impact: Stage B.2/B.3 must decide which markers are true contradictions
  rather than accepted transient, error, or terminal states.

- Observation: the wyvern confirmation pass independently found the same major
  issue families as the local audit: OpenAPI endpoint gaps, AsyncAPI event
  gaps, `ImageAsset` alt-text shape, interests revision shape, idempotency
  scope, auth/sitemap phase mismatch, stack drift, and bottom-nav terminology.
  Evidence: agent `019ec854-d8ae-73b3-bdd7-aa8fd63314ef` completed with a
  candidate list and over-counting warnings.
  Impact: the catalogue uses those families, but keeps the ExecPlan's required
  `FIND-NNNN` identifiers and prescribed labels.

- Observation: the scribe agent team supplied useful prose for introduction,
  coverage, row-update obligation, and caveats, but its draft used a different
  `C-NN` row scheme and non-prescribed labels.
  Evidence: agent `019ec858-25f6-7272-9e92-b092292ce674` completed with a
  draft table using `contract-gap`, `openapi`, and similar labels.
  Impact: integrated only the compatible prose and findings; the committed
  catalogue keeps the canonical row schema and five-label set.

- Observation: Stage B.3 treated many UX orphan markers as candidate signals
  rather than findings. Terminal, transient, future, and error states were
  promoted only when another source exposed a concrete contradiction.
  Evidence: `docs/frontend-source-contradictions-catalogue.md` lists 13
  findings and two duplicate-but-consistent rows rather than one row per
  orphan marker.
  Impact: this avoids the over-counting risk named in the plan while retaining
  traceability to the UX walker output.

- Observation: `make markdownlint` and `make nixie` exposed deterministic
  documentation issues before CodeRabbit. The new catalogue's Markdown tables
  needed alignment, and an existing Mermaid diagram in
  `docs/rstest-bdd-v0-5-0-migration-guide.md` used multi-line node labels that
  Mermaid could not parse.
  Evidence: `/tmp/markdownlint-$(get-project)-$(git branch --show-current).out`
  and `/tmp/nixie-$(get-project)-$(git branch --show-current).out`; both gates
  passed after fixes.
  Impact: the Mermaid repair is an additional documentation change outside the
  planned catalogue set, but it was required to keep the documentation gate
  green and does not settle any front-end finding.

- Observation: the first full `make test` run failed only in
  `backend::catalogue_descriptor_ingestion_bdd` because the embedded
  PostgreSQL bootstrap reported that another server might already be running.
  The same binary passed on a targeted rerun after the full run ended, and the
  later full `make test` rerun passed all suites.
  Evidence:
  `/tmp/test-$(get-project)-$(git branch --show-current).out` reported
  1,285 passed, one failed, and four skipped tests;
  `/tmp/test-rerun-$(get-project)-$(git branch --show-current).out` reported
  9/9 passing tests for the failed binary; and
  `/tmp/test-$(get-project)-$(git branch --show-current)-rerun.out` reported
  the clean full-gate result. No Postgres listener or `postmaster.pid`
  remained after the first full run.
  Impact: treat the first failure as a transient embedded-cluster setup issue.
  CodeRabbit can now review against clean deterministic validation.

- Observation: `make nixie` ran `bun install` and rewrote `bun.lock` even
  though the task did not change package manifests or dependencies.
  Evidence: `git diff -- bun.lock` showed broad dependency churn unrelated to
  the catalogue work. Later stop-hook checks repeatedly found the same
  Bun-resolved lockfile dirty again after restoration.
  Impact: the initial catalogue commit excluded the generated lockfile churn,
  but a separate post-completion commit, `0f25910`, normalizes `bun.lock` so
  repository hooks leave the working tree clean after the validation toolchain
  runs.

- Observation: the Node audit failure was resolved by direct dependency and
  override updates rather than by extending `security/audit-exceptions.json`.
  Evidence: `package.json` now pins patched override versions for esbuild,
  ws, js-yaml, `@babel/core`, DOMPurify, and markdown-it, while Vite is
  declared as `^7.3.5` in each front-end package that names it directly.
  Impact: the audit policy remains strict for these advisories, and future
  pnpm audit failures must still be either patched or explicitly justified.

- Observation: the same `make audit` run that cleared pnpm advisories surfaced
  new RustSec vulnerabilities for `tokio-postgres` `0.7.15` and
  `postgres-protocol` `0.6.9`.
  Evidence: `/tmp/audit-$(get-project)-$(git branch --show-current)-audit-fix.out`
  reported `RUSTSEC-2026-0178`, `RUSTSEC-2026-0179`, and
  `RUSTSEC-2026-0180`; after `cargo update -p tokio-postgres --precise
  0.7.18`, `/tmp/audit-$(get-project)-$(git branch
  --show-current)-audit-fix-rerun.out` reported only the repository's
  existing allowed RustSec warnings.
  Impact: the completed work includes a lockfile-only Rust dependency refresh
  because the requested audit gate could not pass with the previous lockfile.

- Observation: CodeRabbit did not complete for the post-completion audit
  maintenance change even after a retry.
  Evidence:
  `/tmp/coderabbit-$(get-project)-$(git branch --show-current)-audit-fix.out`
  and
  `/tmp/coderabbit-$(get-project)-$(git branch --show-current)-audit-fix-retry.out`
  both reached `preparing_sandbox` and then emitted no findings, completion,
  or rate-limit message before their local processes were terminated.
  Impact: the deterministic gates remain the validation authority for this
  commit; no CodeRabbit concerns were available to resolve.

Append new entries as the audit progresses, citing the relevant file path and
line range as evidence.

## Decision log

Record every significant decision made while working on the plan. Include
decisions to escalate, decisions on ambiguous requirements, and design
choices.

- Decision: the catalogue is published as a single Markdown file at
  `docs/frontend-source-contradictions-catalogue.md`.
  Rationale: the source authority catalogue uses a single file; the two
  documents are designed to be read together, and splitting findings across
  files complicates topic cross-referencing without commensurate review
  benefit. Splitting becomes warranted only if the catalogue exceeds the
  tolerance threshold for candidate volume.
  Date/Author: 2026-06-05, plan draft.

- Decision: the plan is active because the user explicitly requested
  implementation of the planned functionality on 2026-06-14. The approval
  gate in the draft plan is therefore satisfied for this worktree.
  Rationale: the task prompt asks to proceed with implementation and names the
  ExecPlan as the governing plan.
  Date/Author: 2026-06-14, implementation session.

- Decision: use Python/PyYAML instead of `yq` for the scratch AsyncAPI channel
  extraction in Stage B.1.
  Rationale: the plan permits a substitute when `yq` is missing, PyYAML is
  already available, and committing new tooling would breach the
  documentation-only scope.
  Date/Author: 2026-06-14, implementation session.

- Decision: the finding label set is the prescribed five: `update design
  document`, `merge into Progressive Web App design`, `update data model`,
  `write Architecture Decision Record`, `roadmap citation fix`. Contract
  gaps are mapped onto the prescribed labels using the four-branch ownership
  decision tree recorded in Constraints, with `update OpenAPI` and `update
  AsyncAPI` as sub-resolutions.
  Rationale: the roadmap names the label set explicitly. The decision tree
  is necessary because a missing field, a missing endpoint, an encoding
  detail, and a cross-cutting invariant all route to different owners even
  though all surface as "the contract is wrong" in casual review. Without an
  explicit tree the first downstream PR will reopen the labelling debate.
  Date/Author: 2026-06-05, plan draft.

- Decision: the canonical row schema for the catalogue is the YAML block
  below. All references to row fields in Constraints, Stages, and Acceptance
  refer to this schema rather than redescribing the fields each time.

  ```yaml
  - id: FIND-NNNN                 # stable identifier
    topic: <short topic name>
    status: open                  # open | resolved by PR #NNNN |
                                  # deferred | superseded | withdrawn
    severity: blocking            # blocking | important | minor
    label: update design document # one of the prescribed five
    sub_resolution: update OpenAPI # optional; only for contract gaps
    perishability: post-v2a       # pre-v2a | post-v2a
    sources:
      - path: docs/...
        anchor: §<section> or L<line>-L<line>
      - path: spec/...
        anchor: <jsonpointer> or L<line>-L<line>
    claims:
      - source: a                 # a, b — match sources order
        bcp14: MUST                # MUST | SHOULD | MAY | MUST NOT |
                                   # SHOULD NOT | informative
        summary: <one short sentence>
        evidence: "<short quoted excerpt, <=25 words>"
      - source: b
        bcp14: SHOULD
        summary: ...
        evidence: ...
    rationale: <one or two sentences explaining the label choice>
    ownership_note: <one sentence, cites a hexagonal invariant when relevant>
    authority_catalogue_topic: <topic heading in
      docs/frontend-source-authority-catalogue.md>
    siblings: [FIND-NNNN, ...]    # optional; only for umbrella findings
  ```

  Rationale: every prior reference to row fields drifted slightly across
  Constraints, Stage C.2, and Acceptance during drafting; a single canonical
  block prevents accidental schema drift between sections and gives
  downstream owners one place to look. The catalogue itself renders the rows
  as a Markdown table for readability and includes this YAML block in its
  introduction as the normative shape.
  Date/Author: 2026-06-05, plan draft, after Logisphere panel review.

- Decision: each finding uses a stable identifier `FIND-NNNN` so downstream
  PRs can reference the row without depending on line numbers.
  Rationale: the source authority catalogue uses topic headings; the
  contradictions catalogue contains many rows that may be reordered or merged.
  Stable IDs keep downstream PR descriptions and the catalogue itself
  navigable.
  Date/Author: 2026-06-05, plan draft, informed by Kubernetes Enhancement
  Proposal (KEP) front-matter conventions.

- Decision: each finding row carries a `status` field with values `open`,
  `resolved by PR #NNNN`, `deferred`, `superseded`, or `withdrawn`.
  `deferred` covers findings that are genuine but whose downstream work is
  scheduled later (common for `write Architecture Decision Record` rows that
  wait on a phase boundary). `withdrawn` covers audit errors discovered
  after publication.
  Rationale: downstream resolution lands over time; rewriting the catalogue
  is more brittle than appending status. Kubernetes Enhancement Proposal
  status fields ship `deferred` and `withdrawn` because real reconciliation
  programmes need both — the convention also creates a clear audit trail
  when the source authority catalogue and the contradictions catalogue
  diverge.
  Date/Author: 2026-06-05, plan draft, after Logisphere panel review.

- Decision: each finding row carries a `perishability` field with values
  `pre-v2a` or `post-v2a`. `pre-v2a` findings dissolve when the Tailwind
  v3→v4 / DaisyUI v4→v5 stack migration lands and need only a sweep at that
  point; `post-v2a` findings are independent of the stack migration and
  require their own resolution path.
  Rationale: without this axis, the Tailwind/DaisyUI stack drift seeds
  several findings whose lifecycle is dominated by an unrelated downstream
  PR. The flag lets reviewers prioritize durable findings and lets a single
  v2a-migration sweep close the perishable ones in bulk.
  Date/Author: 2026-06-05, plan draft, after Logisphere panel review.

- Decision: closing a finding obliges the PR author to update the affected
  row's `status` field. The obligation is stated in the catalogue's
  introduction and in `docs/developers-guide.md` if that file is amended.
  Rationale: the catalogue is load-bearing only as long as it is current.
  Naming the obligation in both places (the catalogue itself for the row
  author at hand, the developers' guide for contributors who haven't opened
  the catalogue) avoids the most common failure mode for living catalogues.
  Date/Author: 2026-06-05, plan draft, after Logisphere panel review.

- Decision: BCP 14 keywords (MUST, SHOULD, MAY, and negative forms) annotate
  both competing claims in *every* finding. When a source's claim is not
  normative in the BCP 14 sense, the annotation is `informative` (per the
  YAML enum in the row schema).
  Rationale: an earlier draft made the annotation optional. That carve-out
  invited inconsistency — readers could not tell whether an absent
  annotation meant "not useful" or "forgotten". Annotating every claim costs
  minutes per row and pays for itself in disambiguation. BCP 14 (RFC 2119 +
  RFC 8174) restricts normative force to ALL-CAPS keywords, so the
  annotation is grep-friendly and does not pollute the source documents
  themselves; the catalogue performs the rewrite locally for triage.
  Date/Author: 2026-06-05, plan draft, after Logisphere panel review.

- Decision: this work introduces no new linting or static-analysis tooling
  beyond the existing project gates. Existing `make markdownlint`, `make
  nixie`, Spectral on OpenAPI, and AsyncAPI CLI gates remain the verification
  surface for the underlying source documents. No Vale rules, MADR
  generator, shtracer-style harvesters, or LLM diff tools are introduced.
  Rationale: this is a one-off cataloguing pass. New tooling would expand
  scope beyond 0.1.2 and would not survive triage on its own merits.
  Date/Author: 2026-06-05, plan draft.

- Decision: a small state-graph walker is committed under
  `scripts/audit-ux-state-graph.mjs`; the OpenAPI and AsyncAPI identifier
  extraction remains as inline shell in Stage B.1 (not committed).
  Rationale: the state-graph audit catches the F-014-class finding
  (walk-complete entry edge, orphan transitions) that no existing project
  gate covers, and the JSON shape is project-specific. The script's output
  must be reproducible six months from now when the catalogue is amended;
  burying it in `/tmp` would break that. Contract extraction stays inline
  because Spectral and the AsyncAPI CLI already cover the same surface in
  CI; committing parallel scripts would duplicate effort. This reverses an
  earlier blanket "no committed scripts" decision on the state-graph leg
  only.
  Date/Author: 2026-06-05, plan draft, after Logisphere panel review.

- Decision: commit the Bun-resolved `bun.lock` form separately from the
  catalogue implementation after completion.
  Rationale: the plan originally treated lockfile changes as out of scope
  because 0.1.2 did not add or remove dependencies. During validation,
  `make nixie`, `make lint`, and the stop-hook path repeatedly regenerated
  the same lockfile shape after `git restore`. Keeping the file reverted left
  the branch dirty at the stop hook; committing it separately preserves the
  catalogue commit's scope while making the repository state reproducible
  after the required Bun-backed gates. The follow-up commit was gated with
  `make check-fmt`, `make lint`, and `make test`.
  Date/Author: 2026-06-15, post-completion hook reconciliation.

## Outcomes & retrospective

Roadmap item 0.1.2 is complete. The committed catalogue records 13 open
findings and two duplicate-but-consistent rows across the named front-end
source documents and contracts. Each finding carries stable identifiers,
source citations, BCP 14-annotated claims, one approved follow-up label,
status, severity, perishability, ownership notes, and an authority-catalogue
topic cross-reference.

The result matches the original purpose: later phase 1-5 implementation work
can now check whether its cited front-end sources agree, which source owns any
resolution, and whether the fix belongs in design prose, the data model, an
ADR, OpenAPI, AsyncAPI, or roadmap citations. The roadmap item is checked off
with a direct pointer to the catalogue, and the developers guide documents the
row-update obligation for closing PRs.

Two operational lessons matter for future catalogue work. First, the UX
state-graph audit is useful as a triage input, but orphan markers must be
cross-checked against design prose to avoid over-counting generated states as
source contradictions. Second, documentation gates can surface unrelated
syntax breakage; the Mermaid label repair in the rstest-bdd migration guide
was necessary to keep `make nixie` green, but it does not resolve any
front-end finding.

After completion, the stop hook repeatedly reported a dirty `bun.lock` because
the Bun-backed validation tools regenerated the lockfile. Commit `0f25910`
records that normalized lockfile state separately from the catalogue work.
The current branch state is therefore two commits past the draft plan:
`7b743d6` for the catalogue implementation and `0f25910` for lockfile
normalization.

## Context and orientation

Wildside is a Progressive Web App for guided urban walks. The relevant subtree
for this work is documentation, not application code. The front-end source
authority catalogue
(`docs/frontend-source-authority-catalogue.md`) classifies each design
document, schema, and specification as authoritative, supporting, superseded,
or needing reconciliation per topic. The contradictions catalogue produced
here goes one level deeper: it records concrete clashes between specific
paragraphs, schemas, or operations and labels each one with a follow-up
category that names the owning source for resolution.

The primary sources to be audited are the eight named in roadmap item 0.1.2:

- `docs/v2a-front-end-stack.md` — current and target stack reference; takes
  precedence where it conflicts with older PWA platform guidance.
- `docs/building-accessible-and-responsive-progressive-web-applications.md` —
  the accessible-PWA guide that informs service-worker and caching practice.
- `docs/semantic-tailwind-with-daisyui-best-practice.md` — the semantic
  Tailwind/DaisyUI guide.
- `docs/wildside-pwa-design.md` — the Wildside PWA design.
- `docs/wildside-pwa-data-model.md` — the Wildside PWA data model.
- `docs/wildside-ux-state-graph-v0.1.json` — the user experience state graph.
- `docs/sitemap.md` — the planned route paths and navigation groups.
- `spec/openapi.json` and `spec/asyncapi.yaml` — the wire contracts.

Supporting sources cross-referenced during audit include
`docs/wildside-high-level-design.md`,
`docs/data-model-driven-card-architecture.md`, `docs/local-first-react.md`,
`docs/tailwind-v4-guide.md`, `docs/daisyui-v5-guide.md`,
`docs/tailwind-v3-v4-migration-guide.md`,
`docs/pure-accessible-and-localizable-react-components.md`,
`docs/high-velocity-accessibility-first-component-testing.md`,
`docs/enforcing-semantic-tailwind-best-practice.md`,
`docs/react-tailwind-with-bun.md`, `docs/adr-001-websockets-on-actix-ws.md`,
and the per-topic notes in `docs/frontend-source-authority-catalogue.md`.

The Wildside Makefile exposes the gates that apply to documentation-only
changes: `make check-fmt`, `make lint`, `make markdownlint`, `make nixie`, and
`make test`. The semantic linting and Playwright gates referenced by the
roadmap apply to executable front-end work; for a documentation-only
deliverable they are recorded as not applicable in the catalogue.

Hexagonal architecture (Cockburn) underpins how Wildside separates domain
logic from adapter implementations. The contradictions catalogue describes
findings about that boundary using the dependency-rule, port-ownership,
domain-purity, and adapter-isolation invariants documented in the
`hexagonal-architecture` skill. The catalogue itself is not code and does not
itself sit inside the domain or adapter layers.

## Plan of work

Work proceeds in four stages with explicit go/no-go points. Do not advance
to the next stage if the current stage's validation step fails.

Stage A — understand and propose (no committed edits):

- Confirm the label set, finding-identifier scheme, status-field convention,
  and catalogue file path against the Constraints, Tolerances, and Decision
  Log. Capture any divergence in the Decision Log before continuing.
- Read the eight primary sources end-to-end. For each, record audited line
  ranges in the ExecPlan progress notes so coverage is verifiable later.
- Treat the planning research's seeded candidate findings as a *starting
  inventory*, not as authority. Each must be re-verified against the actual
  source text in Stage B.
- Validation: the Decision Log reflects the locked conventions; the audited
  line ranges are recorded; no source file other than this ExecPlan has been
  edited.

Stage B — structured audit (uncommitted scratch work):

- B.1 — Cross-reference contracts to prose. Extract every OpenAPI
  `operationId` and every AsyncAPI channel name with inline shell (output
  kept in `/tmp`, not committed). Walk every UX state-graph state and
  transition with `scripts/audit-ux-state-graph.mjs` (committed), emitting
  for each state its inbound transitions, outbound transitions, and
  matching route(s) from `docs/sitemap.md`. For each identifier and each
  state, search the primary sources for any mention. Orphan identifiers
  (described in prose but missing from contracts, or vice versa) and orphan
  states (no inbound or no outbound, or no route match where the graph
  asserts a route) become candidate findings.
- B.2 — Structured prose audit. Walk each primary source against the hotspot
  priors recorded in `docs/frontend-source-authority-catalogue.md`. For each
  hotspot, formulate a one-sentence claim from each source, apply the BCP 14
  keyword annotation per the row schema, and decide whether the sources
  clash, duplicate, or simply agree.
- B.3 — Triage. For each candidate finding, fill out the row-schema fields
  (label per the ownership decision tree, severity, `perishability`,
  `status` starting at `open` or `deferred` as appropriate, BCP 14
  annotations for both claims). When surviving findings approach 40, apply
  the umbrella-finding rule before continuing: collapse cohesive siblings
  into one umbrella finding with a `siblings:` list. Discard duplicates and
  stylistic variances.
- Validation: the candidate-findings draft lives in the ExecPlan's
  `Surprises & Discoveries`; no source file has been edited yet; surviving
  findings after umbrella collapse fit within the volume tolerance.

Stage C — author the catalogue (committed edits):

- C.1 — Create `docs/frontend-source-contradictions-catalogue.md`. The
  introduction states purpose, scope, the prescribed label set, the
  contract-gap ownership decision tree, the canonical row schema (the YAML
  block from the Decision Log, reproduced verbatim), the status convention
  including `deferred`/`superseded`/`withdrawn`, the `perishability` axis,
  the row-update obligation for closing PRs, the precedence rule from
  `docs/frontend-source-authority-catalogue.md`, and an explicit reminder
  that no finding may be resolved by adding requirement prose only to the
  roadmap.
- C.2 — Add the findings table rendered from the row schema. Order findings
  first by severity (`blocking`, `important`, `minor`) and within each
  severity by label. Render the schema fields as Markdown table columns;
  do not silently drop the `perishability` or BCP 14 annotation columns.
  Add a topic cross-reference table mapping catalogue topics back to the
  source authority catalogue rows. Add a coverage matrix confirming each of
  the eight primary sources has been audited and recording the audited line
  ranges per source in an appendix table (so the ExecPlan itself does not
  carry that volume). Add a validation note mirroring the source authority
  catalogue's pattern, recording that Playwright and `css-view` have no
  rendered UI to validate for this documentation-only deliverable and
  remain required gates for later executable work. Add a relevant-skills
  section pointing to `leta`, `rust-router`, `hexagonal-architecture`,
  `execplans`, and Firecrawl.
- C.3 — Update `docs/developers-guide.md` with a short pointer to the
  catalogue and to the row-update obligation for closing PRs. A one-
  paragraph addition is typically sufficient. This step is no longer
  optional because the row-update obligation is part of the catalogue's
  durability contract.
- Validation: the catalogue file exists at the agreed path, conforms to the
  documentation style guide, and every finding row carries a stable ID,
  label, severity, status, citations, and rationale.

Stage D — verification, roadmap update, and review:

- D.1 — Run `make check-fmt`, `make lint`, `make markdownlint`, and `make
  nixie` sequentially, redirecting output through `tee` to the agreed `/tmp`
  filenames. Resolve any failures, do not silence them.
- D.2 — Run `make test`. The expected outcome is "no functional change in
  test surface". Record the result.
- D.3 — Run `coderabbit review --agent` on the catalogue and any updates to
  `docs/developers-guide.md`. Resolve every concern before requesting human
  review.
- D.4 — Tick roadmap item 0.1.2 in `docs/frontend-roadmap.md`. Add a brief
  pointer to `docs/frontend-source-contradictions-catalogue.md` matching the
  pointer style used for 0.1.1.
- D.5 — Update the PR description with completion notes. Fill in the
  Outcomes & Retrospective section here. Move ExecPlan status to COMPLETE.
- Validation: all listed gates pass on the committed branch; CodeRabbit
  reports no outstanding concerns; the roadmap status accurately reflects
  the work done.

## Concrete steps

Run all commands from the worktree root unless stated otherwise. Use `tee` so
truncation cannot hide failures.

Stage A.1 — confirm conventions and orient:

```bash
git branch --show-current
ls docs/frontend-source-authority-catalogue.md
ls docs/frontend-roadmap.md
```

Expected output (transcript shape):

```plaintext
frontend-0-1-2-catalogue-contradictions-and-duplicated-requirements
docs/frontend-source-authority-catalogue.md
docs/frontend-roadmap.md
```

Stage B.1 — extract identifier inventories. OpenAPI and AsyncAPI extraction
stays inline (the same Spectral and AsyncAPI CLI surface covers them in CI);
the state-graph walker is committed under `scripts/`:

```bash
ACTION=audit
LOG="/tmp/${ACTION}-wildside-$(git branch --show-current).out"
jq -r '.paths | to_entries[] | .key as $p | .value | to_entries[] | "\($p) \(.key) \(.value.operationId // "")"' \
  spec/openapi.json | tee "${LOG}.openapi-ops"
yq -r '.channels | keys[]' spec/asyncapi.yaml | tee "${LOG}.asyncapi-channels"
bun run scripts/audit-ux-state-graph.mjs \
  --graph docs/wildside-ux-state-graph-v0.1.json \
  --sitemap docs/sitemap.md \
  | tee "${LOG}.uxstates"
```

Expected output: each capture writes a sorted list. The state-graph walker
emits, for each state, its inbound and outbound transition counts and any
declared route; orphan states (zero inbound or zero outbound, or a declared
route absent from the sitemap) appear with an `ORPHAN` marker. Missing
tooling (for example, `yq` not installed) is a tolerance trip and the work
pauses to install or substitute.

Stage B.2 — cross-reference prose to identifiers:

```bash
for id in $(sort -u "${LOG}.openapi-ops" "${LOG}.asyncapi-channels" "${LOG}.uxstates"); do
  rg -n --no-heading -F -- "$id" docs/ spec/ || echo "ORPHAN $id"
done | tee "${LOG}.crossref"
```

Expected output: a stream of file:line hits per identifier, with `ORPHAN`
markers for items absent from prose. Capture the orphan list into the
ExecPlan's `Surprises & Discoveries`.

Stage C.1–C.2 — author the catalogue. Use `Write` (or `Edit`) to create
`docs/frontend-source-contradictions-catalogue.md`. Verify the document fits
within the project's 80-column wrap and uses en-GB-oxendict spelling. Use
short, single-language fenced code blocks where examples are needed.

Stage C.3 — optional `docs/developers-guide.md` pointer:

```bash
rg -n "frontend-source-authority-catalogue" docs/developers-guide.md
```

Expected output: if zero hits, consider adding a short pointer; if hits
exist, extend the existing paragraph rather than starting a new section.

Stage D.1 — verification gates:

```bash
ACTION=check-fmt make check-fmt 2>&1 | tee "/tmp/check-fmt-wildside-$(git branch --show-current).out"
ACTION=lint make lint 2>&1 | tee "/tmp/lint-wildside-$(git branch --show-current).out"
ACTION=markdownlint make markdownlint 2>&1 | tee "/tmp/markdownlint-wildside-$(git branch --show-current).out"
ACTION=nixie make nixie 2>&1 | tee "/tmp/nixie-wildside-$(git branch --show-current).out"
```

Expected output: each command exits with status `0`. Any non-zero status
triggers diagnosis under the Tolerances rules; never silence the failure.

Stage D.2 — test gate:

```bash
ACTION=test make test 2>&1 | tee "/tmp/test-wildside-$(git branch --show-current).out"
```

Expected output: exit status `0`. The new catalogue does not introduce
runtime code; the test surface is unchanged.

Stage D.3 — automated review:

```bash
coderabbit review --agent 2>&1 | tee "/tmp/coderabbit-wildside-$(git branch --show-current).out"
```

Expected output: the review either reports no concerns or surfaces concerns
that are addressed before continuing.

Stage D.4 — roadmap status update. Use `Edit` to change the unchecked
checkbox for item 0.1.2 in `docs/frontend-roadmap.md` to a checked checkbox
and add a one-line pointer to `docs/frontend-source-contradictions-catalogue.md`
matching the pointer style used for 0.1.1.

Stage D.5 — commit and update PR:

```bash
git status
git add docs/execplans/frontend-0-1-2-catalogue-contradictions-and-duplicated-requirements.md
# add catalogue, roadmap, and developers-guide if edited
git commit -F /tmp/commit-msg.txt
git push -u origin frontend-0-1-2-catalogue-contradictions-and-duplicated-requirements
```

Use `git commit -F` with a file-based message per the project's commit-message
conventions; do not use `-m`.

## Validation and acceptance

Acceptance is phrased as behaviour a reviewer can verify.

- A reviewer opening `docs/frontend-source-contradictions-catalogue.md`
  finds a brief introduction, a stable finding-identifier scheme, the
  prescribed five-label set, a status convention, and an explicit reminder
  that no finding may be resolved by adding requirement prose only to
  `docs/frontend-roadmap.md`.
- Each finding row matches the canonical row schema recorded in the Decision
  Log and reproduced in the catalogue introduction: a stable `FIND-NNNN`
  identifier, topic, sources, BCP 14-annotated claims with short evidence
  excerpts, exactly one label from the prescribed set (with sub-resolution
  for contract gaps), rationale, severity, status (one of `open`, `resolved
  by PR #NNNN`, `deferred`, `superseded`, `withdrawn`), `perishability`
  (`pre-v2a` or `post-v2a`), ownership note, and authority-catalogue topic
  cross-reference. Umbrella findings additionally carry a `siblings:` list.
- A reviewer can take any finding, open the cited source files, and verify
  both quoted fragments resolve to the cited locations.
- A reviewer can take any phase 1-5 implementation task in
  `docs/frontend-roadmap.md` that cites one of the audited sources and check
  the catalogue's topic cross-reference table to see whether the task's
  cited authority is currently coherent.
- `docs/frontend-roadmap.md` shows item 0.1.2 checked off with a pointer to
  the new catalogue.
- `make check-fmt`, `make lint`, `make markdownlint`, `make nixie`, and
  `make test` all exit with status `0` on the final commit.
- `coderabbit review --agent` reports no outstanding concerns.

Quality criteria (what "done" means):

- Tests: `make test` passes. No new tests are introduced because no runtime
  behaviour changes; if the surface changes during implementation, stop and
  escalate.
- Lint and typecheck: `make check-fmt`, `make lint`, `make markdownlint`, and
  `make nixie` pass.
- Documentation: the catalogue conforms to `docs/documentation-style-guide.md`
  (en-GB-oxendict, 80-column wrap, language-tagged code fences, Mermaid prose
  alternatives where Mermaid is used).
- Accessibility: not applicable to this documentation-only deliverable. The
  catalogue's validation note records that Playwright and `css-view` have no
  rendered UI to validate for this item, and remain required gates for later
  executable front-end work.
- Hexagonal boundaries: every finding that touches the domain-adapter
  boundary explains ownership using the `hexagonal-architecture` skill's
  invariants and does not couple front-end code to backend adapter
  internals. The two concrete probes used during audit are (a) any entity
  schema that ships presentation-layer fields (CSS class names, gradients,
  pre-rendered HTML), which violates domain purity, and (b) any front-end
  reliance on backend WebSocket framing details beyond what
  `docs/adr-001-websockets-on-actix-ws.md` exposes as an inbound adapter
  contract, which violates adapter isolation.

Quality method (how we check):

- Sequential `make` invocations with `tee` capture under `/tmp` as above.
- Manual reviewer pass against the acceptance bullets above.
- CodeRabbit review on the committed catalogue.

## Idempotence and recovery

All steps are repeatable.

- The scratch identifier-inventory commands write to `/tmp` and may be rerun
  freely. They produce no committed artefacts.
- The catalogue file is authored once and amended through normal git edits.
  If the file is overwritten in error, `git restore` recovers the previous
  state.
- The roadmap checkbox update is reversed by ticking the box back to
  unchecked and removing the pointer line.
- The verification gates are pure status checks; re-running them is
  side-effect free.
- The branch may be force-pushed only on the working branch, never on
  `main`. If a force-push is needed, document the reason in the Decision
  Log first.

## Artefacts and notes

The plan-time research dispatch surfaced an initial inventory of roughly 28
candidate findings spanning schema-shape clashes (image alt localization,
backend CSS classes on entity schemas, parallel offline-bundle entities),
contract gaps (catalogue and route-generation endpoints absent from OpenAPI,
route-generation and offline-bundle progress events absent from AsyncAPI),
state-machine and routing intent (auth route presence, wizard-step-3 outbound
target, walk-complete entry edge, bottom-nav `Discover` versus `Explore`
naming), and platform-policy items already named by the source authority
catalogue (client-state ownership, service-worker update strategy, outbox
retry semantics, map tile provider strategy). These are seeds, not
conclusions; Stage B must re-verify each against the actual source text
before any candidate enters the committed catalogue.

When the catalogue is committed, its evidence section captures short quoted
excerpts only. Do not redistribute large source-document fragments.

## Interfaces and dependencies

This work produces a single Markdown deliverable, amends two other Markdown
files, and commits one auditing script. No code interfaces affecting runtime
behaviour are added or changed. No new runtime dependencies are introduced;
the script under `scripts/` is run via Bun (already a project dependency)
and only reads JSON.

The committed script is `scripts/audit-ux-state-graph.mjs`. Its contract:

- Inputs: `--graph <path-to-json>` and `--sitemap <path-to-markdown>`.
- Output (stdout): one line per state, formatted as
  `<state-id> in=<N> out=<M> route=<route-or-NONE> [ORPHAN]`.
- Exit code: `0` if the script completes; `1` only on input errors (file
  not found, malformed JSON). Orphan findings are *not* errors; they are
  data for human triage.
- The script is pure: no network, no file writes, no environment reads
  beyond its CLI arguments.

Relevant skills:

- `leta` — used for any code-symbol probe when verifying whether a cited
  source actually references a named symbol in `frontend-pwa/`. Workspace
  must be added to leta before code probes (`leta workspace add ...`).
- `rust-router` — referenced as a pointer for future implementation work; not
  used directly here because this catalogue does not introduce Rust code.
- `hexagonal-architecture` — applied to findings that touch domain-versus-
  adapter ownership. The catalogue describes those findings using the
  dependency-rule, port-ownership, domain-purity, and adapter-isolation
  invariants.
- `execplans` — owns the structure of this document.
- `documentation-style-guide` — owns wrap, spelling, code-fence, and Mermaid
  rules.
- Firecrawl — used during plan drafting to scan prior art on requirements
  traceability matrices, ADR templates, BCP 14 keyword annotation, Diátaxis,
  and contract-linting tooling. The catalogue does not introduce any of the
  scanned tools; existing project gates (Spectral, AsyncAPI CLI,
  markdownlint, nixie) remain the verification surface for the underlying
  source documents.

Signposted source documents that the catalogue must cite by name (not an
exhaustive list):

- `docs/frontend-roadmap.md` — the queue this work feeds.
- `docs/frontend-source-authority-catalogue.md` — the authority map the
  contradictions catalogue extends.
- `docs/wildside-pwa-design.md`, `docs/wildside-pwa-data-model.md`,
  `docs/wildside-ux-state-graph-v0.1.json`, `docs/sitemap.md`, and the
  v2a-stack/accessible-PWA/semantic-Tailwind guides — the audited sources.
- `spec/openapi.json` and `spec/asyncapi.yaml` — the audited wire contracts.
- `docs/documentation-style-guide.md` — governs Markdown rules.
- `docs/v2a-front-end-stack.md` — provides the precedence rule.
- `docs/data-model-driven-card-architecture.md`,
  `docs/local-first-react.md`, `docs/pure-accessible-and-localizable-react-components.md`,
  `docs/high-velocity-accessibility-first-component-testing.md`,
  `docs/enforcing-semantic-tailwind-best-practice.md` — supporting documents
  cross-referenced during audit.
- `docs/adr-001-websockets-on-actix-ws.md` — the only existing ADR that
  affects the audit boundary.

## Revision note

- 2026-06-05 — Initial draft.
- 2026-06-05 — Revised after Logisphere panel review. Locked the canonical
  row schema as a Decision Log YAML block; added the four-branch contract-gap
  ownership decision tree to Constraints with worked examples; promoted the
  state-graph walker to a committed script under `scripts/`; added
  `deferred` and `withdrawn` to the status convention; added a
  `perishability` axis (`pre-v2a` / `post-v2a`); added the row-update
  obligation for closing PRs; made BCP 14 annotation mandatory for every
  claim; tightened Stage B.3's umbrella-finding rule to fire before the
  tolerance threshold; sharpened the hexagonal probes; added a durable
  six-month success vision to Purpose. These revisions do not change the
  plan's scope or the prescribed five-label set; they harden the row schema
  so downstream owners do not relitigate it in the first closing PR.
