# Build the front-end source authority catalogue

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item 0.1.1 creates a source authority catalogue for Wildside front-end
requirements before feature implementation begins. After this work is complete,
a contributor can open one catalogue and see which document owns each platform,
data, user experience state, Application Programming Interface (API), styling,
accessibility, localization, and testing requirement, and which topics still
need a design document or Architecture Decision Record (ADR) update.

This plan is for the catalogue work only. It must be approved before
implementation begins. The catalogue is expected to be documentation-first and
must not introduce front-end runtime behaviour, user interface strings, card
model data, dependencies, or API contract changes without explicit approval.

## Constraints

The implementation must satisfy `docs/frontend-roadmap.md` item 0.1.1 and must
not implement later roadmap items 0.1.2 through 0.3.2. It may identify
contradictions and follow-up work, but it must not resolve those contradictions
unless the resolution is limited to naming the owning source or follow-up.

`docs/v2a-front-end-stack.md` takes precedence where it conflicts with older
Progressive Web Application (PWA) platform guidance. Older PWA documents may be
classified as supporting, superseded for a topic, or needing reconciliation;
the catalogue must not silently merge incompatible guidance.

The roadmap remains an implementation queue, not a design authority. Product
policy, schema shape, platform policy, and user experience rules must live in
design documents, ADRs, `spec/openapi.json`, `spec/asyncapi.yaml`, or the user
experience state graph. The roadmap may cite those sources and track work.

Hexagonal architecture boundaries must be preserved. The catalogue must
distinguish domain and policy documents from port and adapter documents, and it
must not ask front-end code to depend on backend adapter internals. API
contracts are consumed through OpenAPI, AsyncAPI, generated client boundaries,
or documented ports.

The future implementation must use en-GB-oxendict spelling and the Markdown
rules in `docs/documentation-style-guide.md`. Paragraphs and bullets should
wrap at 80 columns, code blocks must declare a language, and Mermaid diagrams
must include accessible prose where diagrams are added.

No new front-end runtime strings or card model data are expected for this
catalogue. If the implementer discovers that a user interface or fixture change
is required, every new string and every card-model label must be translatable
according to `docs/v2a-front-end-stack.md`, all supported locales and
right-to-left (RTL) behaviour must be covered, and implementation must stop for
approval before adding runtime dependencies or translation infrastructure.

Wildside targets Web Content Accessibility Guidelines (WCAG) 2.2 Level AA for
front-end work. For this documentation-only catalogue, the acceptance criterion
is that the catalogue names the authoritative accessibility source and the
future validation gates. Any executable front-end change discovered during
implementation must pass browser-level validation with no errors, failures, or
accessibility violations before commit.

Run validation commands sequentially and capture long output with `tee` under
`/tmp`. Do not run format, lint, or test commands in parallel. Prefer Makefile
targets over package-local commands.

Commit only after the relevant gates pass. Use a file-based commit message via
`git commit -F`; do not use `git commit -m`.

## Tolerances

Escalate before continuing if implementing this plan requires changing more
than three source documents other than the catalogue, the roadmap checkbox, and
this ExecPlan. The expected steady-state edit set is small: a new catalogue
document, this ExecPlan updates, a roadmap status update when the catalogue is
implemented, and possibly `docs/developers-guide.md` if contributor workflow
guidance changes.

Escalate if any backend Rust, TypeScript source, package manifest, lockfile,
OpenAPI spec, AsyncAPI spec, generated artefact, or migration must change.
Roadmap item 0.1.1 is about authority classification, not contract or runtime
implementation.

Escalate if the source authority catalogue cannot name one authoritative source
or one reconciliation follow-up for every platform, data, user
experience-state, API, and styling topic referenced by the roadmap.

Escalate if Playwright or `css-view` validation is impossible for a reason
other than "no executable front-end surface changed". If no executable surface
changed, record that fact in the catalogue or ExecPlan evidence and still run
the available documentation and repository gates.

Escalate if `make check-fmt`, `make lint`, or `make test` fails twice after
clear, relevant fixes. Do not work around failures by narrowing the gate or
silencing lints.

Escalate if `coderabbit review --agent` reports a concern that cannot be
resolved without expanding the scope beyond this catalogue.

## Risks

Risk: the catalogue becomes a hidden design document. Severity: high.
Likelihood: medium. Mitigation: phrase entries as ownership classifications and
follow-up labels, not as new product policy. If new policy is needed, name an
ADR or design document follow-up.

Risk: the current repository differs from the fuller v2a target stack.
Severity: high. Likelihood: high. Mitigation: classify current package state
separately from target design authority. `docs/v2a-front-end-stack.md` already
distinguishes the checked-in stack from the target stack, and the catalogue
must preserve that distinction.

Risk: requested validation tooling is target-state rather than installed
project state. Severity: medium. Likelihood: high. Mitigation: record which
gates are executable today and which are roadmap follow-ups. `css-view` is
present on the machine; Playwright is not currently declared in
`frontend-pwa/package.json`. Do not add Playwright solely for this catalogue
without approval.

Risk: API design intent and implemented API specs differ. Severity: high.
Likelihood: high. Mitigation: classify `spec/openapi.json` and
`spec/asyncapi.yaml` as authoritative for implemented wire contracts, and
classify richer PWA design endpoint/event expectations as reconciliation
follow-ups.

Risk: there is no generic `docs/users-guide.md` in this repository. Severity:
low. Likelihood: high. Mitigation: because this catalogue should not change
user-facing behaviour, do not create a user guide only to say nothing changed.
Record the absence and update the relevant user guide only if implementation
introduces user-visible tool behaviour, which is outside the expected scope.

## Progress

- [x] (2026-05-20T18:18:15Z) Load the requested `leta`, `rust-router`, and
  `hexagonal-architecture` skills, and create a `leta` workspace for this
  worktree.
- [x] (2026-05-20T18:18:15Z) Rename the branch to
  `frontend-0-1-1-front-end-source-authority-catalogue`.
- [x] (2026-05-20T18:18:15Z) Use Wyvern agents to inspect source authority,
  validation tooling, and ADR/design-document constraints.
- [x] (2026-05-20T18:18:15Z) Use Firecrawl to check current external context
  for WCAG 2.2, Playwright accessibility testing, `css-view` discoverability,
  and LemmaScript prior art.
- [x] (2026-05-20T18:18:15Z) Draft this task-specific ExecPlan.
- [x] (2026-05-20T18:22:00Z) Run `make markdownlint` and fix the only
  Markdown wrapping issue in this ExecPlan.
- [x] (2026-05-20T18:23:00Z) Attempt `coderabbit review --agent`; the command
  reached the review service and stopped on a service-side usage-credit rate
  limit before returning findings.
- [x] (2026-05-20T18:31:00Z) Validate this draft ExecPlan with
  `make check-fmt`, `make lint`, `make test`, and `make nixie`.
- [x] (2026-05-20T18:36:00Z) Probe `css-view` and Playwright availability for
  this plan-only change. `css-view --help` succeeded, and
  `bunx playwright --version` reported version 1.60.0, but no executable
  front-end surface was changed.
- [x] (2026-05-20T19:43:26Z) Receive explicit user approval to implement
  roadmap item 0.1.1 from this ExecPlan.
- [x] (2026-05-20T19:45:00Z) Draft
  `docs/frontend-source-authority-catalogue.md` with source classifications,
  topic authorities, reconciliation follow-ups, skills, and validation notes.
- [x] (2026-05-20T19:45:00Z) Mark roadmap item 0.1.1 done and cite the
  catalogue from `docs/frontend-roadmap.md`.
- [x] (2026-05-20T19:45:00Z) Add the catalogue to
  `docs/developers-guide.md` and `docs/contents.md`.
- [x] (2026-05-20T19:46:00Z) Run `make fmt` and `make markdownlint`; both
  passed for the documentation update.
- [x] (2026-05-20T19:52:00Z) Run `coderabbit review --agent`; the review
  completed successfully with zero findings.
- [x] (2026-05-20T19:53:00Z) Run `make nixie`; all diagrams validated
  successfully.
- [x] (2026-05-20T19:53:00Z) Run `css-view --help`; the installed command is a
  Playwright-backed CSS snapshot CLI that requires a URL, and no executable
  front-end surface changed in this documentation-only item.
- [x] (2026-05-20T19:57:00Z) Run `make check-fmt`, `make lint`, and
  `make test`; all repository gates passed.
- [x] (2026-05-20T19:58:28Z) Commit the implementation, push the branch, and
  update draft pull request #355.

## Surprises & discoveries

- Observation: `docs/execplans/frontend-phase-0-source-reconciliation.md`
  already exists as a broader phase-0 plan. Evidence: the file describes source
  reconciliation across phase 0 and names `docs/v2a-front-end-stack.md` as the
  precedence source. Impact: this ExecPlan stays narrower and task-specific for
  roadmap item 0.1.1 instead of duplicating the phase-level plan.

- Observation: only one ADR exists, and it is backend WebSocket transport
  policy. Evidence: `docs/adr-001-websockets-on-actix-ws.md` is the only ADR
  found under `docs/`. Impact: the catalogue will likely name several ADR
  follow-ups rather than citing existing ADRs for front-end stack, local-first,
  accessibility, localization, and styling governance.

- Observation: Playwright is not currently declared in the front-end package,
  while `css-view` is installed on the machine. Evidence:
  `frontend-pwa/package.json` has no Playwright dependency, and
  `command -v css-view` resolves to `/home/leynos/.bun/bin/css-view`. Impact:
  this plan treats Playwright as a required future front-end validation gate
  when executable front-end work lands, not as a dependency to introduce during
  catalogue drafting.

- Observation: external search for a `css-view` project did not identify a
  clear upstream documentation source. Evidence: Firecrawl search for
  `"css-view" CSS CLI GitHub npm` returned unrelated CSS View Transition and
  CSS CLI results. Impact: the implementation should validate the local command
  directly and record its invocation, rather than relying on uncertain external
  prior art.

- Observation: CodeRabbit was available locally but could not complete a
  review because the remote service reported a usage-credit rate limit.
  Evidence: `coderabbit review --agent` emitted `errorType: "rate_limit"` and
  no actionable findings. Impact: this draft has local validation evidence but
  no CodeRabbit findings to resolve until credits are available or the review
  is run in another environment.

- Observation: probing Playwright with `bunx playwright --version` attempted
  dependency resolution and rewrote `bun.lock`. Evidence: the command reported
  Playwright 1.60.0 and changed the `ip-address` override in `bun.lock`.
  Impact: the lockfile change was reverted because this plan-only branch must
  not carry incidental dependency churn. Future executable front-end validation
  should use the repository's committed Playwright command once one exists.

- Observation: `docs/developers-guide.md` named `make build-frontend`, but the
  repository Makefile exposes the front-end build target as `make fe-build`.
  Evidence: the Makefile has an `fe-build` target and no `build-frontend`
  target. Impact: the implementation updates the developers' guide while adding
  the new source-authority reference so contributors receive executable
  workflow guidance.

- Observation: `css-view` is available but requires a page URL and snapshots a
  rendered page through Playwright. Evidence: `css-view --help` prints
  `Usage: css-view [options] <url>` and describes capturing computed CSS
  snapshots for a page. Impact: item 0.1.1 changes only documentation and has
  no rendered front-end surface for `css-view` or Playwright to exercise. The
  catalogue preserves those tools as mandatory future gates for executable
  front-end changes.

## Decision log

- Decision: keep this ExecPlan in DRAFT until the user explicitly approves
  implementation. Rationale: the `execplans` skill requires an approval gate,
  and the user explicitly stated that the plan must be approved before it is
  implemented. Date/Author: 2026-05-20T18:18:15Z / Codex.

- Decision: classify `docs/v2a-front-end-stack.md` as the precedence source
  for front-end stack conflicts. Rationale: roadmap item 0.1.1 explicitly
  requires this rule, and the broader phase-0 ExecPlan already encodes the same
  constraint. Date/Author: 2026-05-20T18:18:15Z / Codex.

- Decision: treat OpenAPI and AsyncAPI specs as authoritative for implemented
  wire contracts, while treating PWA design documents as contract intent when
  specs lag. Rationale: generated or authored specs are the executable contract
  boundary; richer design expectations that are absent from specs must become
  reconciliation follow-ups. Date/Author: 2026-05-20T18:18:15Z / Codex.

- Decision: do not mark roadmap item 0.1.1 complete while drafting this plan.
  Rationale: the source authority catalogue itself has not been implemented;
  marking the roadmap item done during plan review would misrepresent project
  state. Date/Author: 2026-05-20T18:18:15Z / Codex.

- Decision: create a draft pull request for this ExecPlan before catalogue
  implementation. Rationale: the user requested a reviewable plan and
  explicitly required plan approval before implementation. The implementation
  pull request can update or follow this draft after approval. Date/Author:
  2026-05-20T18:36:00Z / Codex.

- Decision: move this ExecPlan from DRAFT to IN PROGRESS after explicit user
  approval. Rationale: the user asked to proceed with implementation of this
  approved plan, so the approval gate has been satisfied and roadmap item 0.1.1
  can now be implemented within the recorded tolerances. Date/Author:
  2026-05-20T19:43:26Z / Codex.

- Decision: update `docs/developers-guide.md` and `docs/contents.md` alongside
  the catalogue. Rationale: the catalogue changes contributor navigation and
  documentation ownership for front-end work, so the developer guide and
  documentation index should point to it. The developers' guide update also
  corrects the existing front-end build target name to match the Makefile.
  Date/Author: 2026-05-20T19:45:00Z / Codex.

## Outcomes & retrospective

The catalogue implementation is complete. The branch has been pushed and draft
pull request #355 has been updated for implementation review.

For the pre-approval plan review, the following commands have passed:

- `make markdownlint`
- `make check-fmt`
- `make lint`
- `make test`
- `make nixie`
- `css-view --help`

CodeRabbit could not complete because the service returned a usage-credit rate
limit before reporting findings. Playwright was available through `bunx` and
reported version 1.60.0, but there was no rendered front-end surface to test
for this plan-only change.

During implementation, CodeRabbit completed successfully for the catalogue
draft and reported zero findings.

The implementation created `docs/frontend-source-authority-catalogue.md`,
marked roadmap item 0.1.1 done in `docs/frontend-roadmap.md`, added the
catalogue to `docs/developers-guide.md` and `docs/contents.md`, and kept this
ExecPlan current.

The following implementation validation commands have passed:

- `make fmt`
- `make markdownlint`
- `make nixie`
- `css-view --help`
- `coderabbit review --agent`
- `make check-fmt`
- `make lint`
- `make test`

## Context and orientation

The repository root is
`/home/leynos/.lody/repos/github---leynos---wildside/worktrees/7a65440e-31f7-4e2f-b9e2-9e4348e8ce5b`.
The current branch for this work is
`frontend-0-1-1-front-end-source-authority-catalogue`.

The requested roadmap item is in `docs/frontend-roadmap.md` under "0.1.
Catalogue authority, overlaps, and contradictions". Item 0.1.1 requires a
front-end source authority catalogue that classifies these sources by topic:

- `docs/v2a-front-end-stack.md`
- `docs/wildside-pwa-design.md`
- `docs/wildside-pwa-data-model.md`
- `docs/wildside-ux-state-graph-v0.1.json`
- `docs/sitemap.md`
- `spec/openapi.json`
- `spec/asyncapi.yaml`
- relevant ADRs

Use the word "authoritative" for the document that owns a requirement. Use
"supporting" for documents that provide background or implementation guidance
but do not own the requirement. Use "superseded" only for guidance that should
no longer be followed for a topic. Use "needs reconciliation" when the
documents do not agree or when a design document, ADR, schema, or contract must
be updated before implementation work can cite a stable authority.

The broader phase-0 planning file
`docs/execplans/frontend-phase-0-source-reconciliation.md` is relevant
background, but this plan must remain self-contained and task-specific. A
future implementer should be able to complete 0.1.1 from this file alone.

## Source authority findings to preserve

For platform topics, the catalogue should treat `docs/v2a-front-end-stack.md`
as authoritative for the current-versus-target stack split and precedence over
older PWA guidance. `docs/wildside-pwa-design.md` is authoritative for Wildside
PWA runtime behaviour where it does not conflict with that stack precedence.

For data topics, the catalogue should treat `docs/wildside-pwa-data-model.md`
as authoritative for entity shapes, localization maps, offline bundle models,
outbox concepts, and the backend-compatible card model.
`docs/v2a-front-end-stack.md` and `docs/data-model-driven-card-architecture.md`
are supporting sources for the shared card architecture and localization
primitive pattern.

For user experience state, the catalogue should treat
`docs/wildside-ux-state-graph-v0.1.json` as authoritative for state regions,
transitions, and state metadata, with `docs/sitemap.md` supporting route
structure and navigation groups. The state graph includes auth and future
states that are not explicit in the sitemap, so those topics need
reconciliation follow-ups.

For API topics, the catalogue should treat `spec/openapi.json` as authoritative
for implemented REST wire contracts and `spec/asyncapi.yaml` as authoritative
for implemented WebSocket/event contracts. Design documents may describe
intended endpoint families and progress events that are not yet present in the
specs; those are reconciliation follow-ups, not implemented contract authority.

For styling, the catalogue should treat `docs/v2a-front-end-stack.md` and
`docs/wildside-pwa-design.md` as authoritative for the current-versus-target
styling stack, token pipeline, and semantic styling model. The supporting
styling guides are `docs/tailwind-v4-guide.md`, `docs/daisyui-v5-guide.md`,
`docs/semantic-tailwind-with-daisyui-best-practice.md`, and
`docs/enforcing-semantic-tailwind-best-practice.md`.

For accessibility, the catalogue should treat `docs/wildside-pwa-design.md` as
the Wildside accessibility requirement source and
`docs/high-velocity-accessibility-first-component-testing.md` as the testing
strategy source. External context confirms WCAG 2.2 is a W3C Recommendation and
that WCAG 2.2 success criteria are testable statements. Playwright's official
accessibility guide recommends combining automated checks with manual and
inclusive assessment because automated tests cannot catch all accessibility
problems.

For localization and RTL, the catalogue should treat
`docs/wildside-pwa-design.md`, `docs/v2a-front-end-stack.md`, and
`docs/wildside-pwa-data-model.md` as the authority set. UI chrome belongs in
translation resources, entity display text belongs in entity localizations,
unsupported locale tags fall back to `en-GB`, and RTL support should rely on
logical CSS properties plus MapLibre RTL text support when maps are present.

For testing, the catalogue should separate current executable gates from
target-state gates. Current Makefile gates include `make check-fmt`,
`make lint`, `make test`, `make markdownlint`, and `make nixie`. Target-state
front-end validation includes Playwright, axe-driven accessibility checks,
semantic CSS linting, localization linting, behavioural tests with Gherkin
support, property tests with `fast-check`, and proof tooling such as
LemmaScript when contractual business logic introduces axioms.

## Plan of work

After approval, start by rereading this ExecPlan, `AGENTS.md`,
`docs/frontend-roadmap.md`, and `docs/documentation-style-guide.md`. Confirm
the branch is still `frontend-0-1-1-front-end-source-authority-catalogue` and
that the worktree has no unrelated edits that would be swept into the catalogue
commit.

Create a new document named `docs/frontend-source-authority-catalogue.md`. Use
a short purpose section, a classification legend, a topic-by-topic authority
catalogue, and a reconciliation follow-up section. Keep the document factual.
Each topic entry must name one authoritative source or one named follow-up.
Each follow-up must say whether it belongs in a design document, ADR, OpenAPI
spec, AsyncAPI spec, roadmap citation fix, or later implementation task.

Cover at least these topics: runtime and build stack, routing, state
management, local-first persistence, PWA installability, service worker and
caching policy, map stack, data model and card model, localization and RTL,
accessibility, styling and tokens, REST contracts, WebSocket/event contracts,
UX state graph, sitemap routes, testing and validation, semantic linting,
property tests, proof obligations, documentation ownership, and hexagonal
boundary ownership.

Preserve the known contradictions as reconciliation follow-ups. The OpenAPI
spec lacks several endpoint families described by the PWA data model. The
AsyncAPI spec documents narrower WebSocket messages than the route-generation
progress expectations in the design and state graph. The state graph includes
auth and future states not represented in the sitemap. The current
`frontend-pwa/package.json` uses Tailwind CSS `^3` and DaisyUI `^4`, while the
target stack points to Tailwind CSS v4 and DaisyUI v5.

Update `docs/frontend-roadmap.md` only after the catalogue itself satisfies the
success criterion. Change item 0.1.1 from `[ ]` to `[x]` and add a citation to
`docs/frontend-source-authority-catalogue.md` if the roadmap wording needs a
stable link. Do not mark later phase-0 items complete.

Update `docs/developers-guide.md` if the implementation changes contributor
workflow, document ownership, or validation expectations. If no workflow
changes are made, record that no developer guide update was required in this
ExecPlan's `Decision Log`. Do not create `docs/users-guide.md` for this
catalogue unless the implementation introduces user-visible behaviour; it
should not.

Run CodeRabbit after the catalogue is drafted and before final commit:

```bash
coderabbit review --agent
```

Resolve all actionable concerns within scope. If CodeRabbit asks for
implementation beyond roadmap item 0.1.1, record it as a follow-up or escalate.

Finish by running validation sequentially, committing, pushing the branch to
`origin/frontend-0-1-1-front-end-source-authority-catalogue`, and creating a
draft pull request whose title includes `(frontend-0.1.1)` and whose summary
links this ExecPlan.

## Concrete steps

From the repository root, verify branch and worktree state:

```bash
git branch --show-current
git status --short
```

Expected branch output:

```plaintext
frontend-0-1-1-front-end-source-authority-catalogue
```

Refresh local context with text searches because the source files for this task
are Markdown, JSON, YAML, and package metadata rather than code symbols:

```bash
rg -n \
  "0\\.1\\.1|v2a|PWA|WCAG|Playwright|css-view|LemmaScript|fast-check|rtl|localization|localisation" \
  docs spec Makefile package.json frontend-pwa packages
```

Draft the catalogue with `apply_patch`. Do not use shell redirection to create
or edit repository files.

Validate Markdown formatting before broader gates:

```bash
make fmt 2>&1 | tee "/tmp/fmt-wildside-$(git branch --show-current).out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-wildside-$(git branch --show-current).out"
```

If the catalogue adds or changes Mermaid diagrams, validate them:

```bash
make nixie 2>&1 | tee "/tmp/nixie-wildside-$(git branch --show-current).out"
```

Run front-end and repository gates sequentially:

```bash
make check-fmt 2>&1 | tee "/tmp/check-fmt-wildside-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-wildside-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-wildside-$(git branch --show-current).out"
```

For `css-view`, first record the local command contract, then use the command
against the front-end CSS or built output if it supports that mode. If it does
not support a useful docs-only validation mode, record that no executable
front-end surface changed and that `css-view` has no relevant input for this
catalogue.

```bash
css-view --help 2>&1 | tee "/tmp/css-view-help-wildside-$(git branch --show-current).out"
```

For Playwright, do not add a dependency during catalogue implementation unless
the approved scope is expanded. If Playwright is already available by the time
implementation starts, run the repository's Playwright command or a targeted
accessibility smoke. If it is still absent and no UI changed, record that the
catalogue signposts Playwright as a future executable gate but has no runtime
surface to test.

Run CodeRabbit:

```bash
coderabbit review --agent 2>&1 | tee "/tmp/coderabbit-wildside-$(git branch --show-current).out"
```

Inspect the diff before staging:

```bash
git diff -- docs/frontend-source-authority-catalogue.md docs/frontend-roadmap.md docs/developers-guide.md docs/execplans/frontend-0-1-1-front-end-source-authority-catalogue.md
```

Commit with a file-based message:

```bash
COMMIT_MSG_DIR=$(mktemp -d)
cat > "$COMMIT_MSG_DIR/COMMIT_MSG.md" << 'ENDOFMSG'
Catalogue front-end source authority

Add the source authority catalogue for roadmap item 0.1.1 and mark the
roadmap entry complete once every front-end topic has an owning source or
named reconciliation follow-up.
ENDOFMSG
git add docs/frontend-source-authority-catalogue.md docs/frontend-roadmap.md docs/execplans/frontend-0-1-1-front-end-source-authority-catalogue.md
git add docs/developers-guide.md
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

Push and open a draft pull request only after validation and CodeRabbit review
are clean:

```bash
git push -u origin frontend-0-1-1-front-end-source-authority-catalogue
echo "${LODY_SESSION_ID}"
```

Use the printed session identifier to include this link in the pull request
body:

```plaintext
https://lody.ai/leynos/sessions/${LODY_SESSION_ID}
```

## Validation and acceptance

The catalogue is accepted when every platform, data, user experience-state,
API, styling, accessibility, localization, and testing topic referenced by
`docs/frontend-roadmap.md` has exactly one named authoritative source or a
named reconciliation follow-up in `docs/frontend-source-authority-catalogue.md`.

The catalogue must explicitly state that `docs/v2a-front-end-stack.md` takes
precedence where it conflicts with older PWA platform guidance.

The catalogue must classify all required source files:
`docs/v2a-front-end-stack.md`, `docs/wildside-pwa-design.md`,
`docs/wildside-pwa-data-model.md`, `docs/wildside-ux-state-graph-v0.1.json`,
`docs/sitemap.md`, `spec/openapi.json`, `spec/asyncapi.yaml`, and
`docs/adr-001-websockets-on-actix-ws.md`.

The catalogue must signpost relevant supporting documentation and skills:
`leta`, `rust-router`, `hexagonal-architecture`, `execplans`, Firecrawl,
`docs/tailwind-v4-guide.md`,
`docs/semantic-tailwind-with-daisyui-best-practice.md`,
`docs/react-tailwind-with-bun.md`,
`docs/pure-accessible-and-localizable-react-components.md`,
`docs/local-first-react.md`,
`docs/high-velocity-accessibility-first-component-testing.md`,
`docs/enforcing-semantic-tailwind-best-practice.md`,
`docs/data-model-driven-card-architecture.md`, `docs/daisyui-v5-guide.md`, and
`complexity-antipatterns-and-refactoring-strategies.md` if present.

Documentation validation must pass:

```plaintext
make fmt
make markdownlint
make nixie
```

Repository gates must pass:

```plaintext
make check-fmt
make lint
make test
```

CodeRabbit must report no unresolved actionable concerns for the catalogue
scope. Any concern that belongs to a later roadmap item must be recorded as a
follow-up rather than ignored.

If no executable front-end surface changes, Playwright and `css-view`
validation are accepted by recording that the catalogue has no rendered UI to
exercise and by preserving those tools as required gates for future executable
front-end work. If any executable front-end surface changes, Playwright and
`css-view` must run successfully with no errors, failures, or accessibility
violations before commit.

## Idempotence and recovery

The catalogue work is safe to rerun because it is documentation-only. If
formatting changes wrap Markdown differently, rerun `make fmt` and inspect the
diff before staging.

If validation fails because of pre-existing unrelated changes, do not revert
those changes. Capture the failing log under `/tmp`, identify whether the
failure is related to the catalogue, and escalate if the failure blocks a clean
commit.

If the branch loses its upstream during push, set the upstream explicitly with:

```bash
git push -u origin frontend-0-1-1-front-end-source-authority-catalogue
```

If a pull request already exists for this branch, update its title and body
instead of opening a duplicate.

## Interfaces and dependencies

No runtime interfaces or package dependencies should be introduced by this
catalogue. The only expected new stable document interface is
`docs/frontend-source-authority-catalogue.md`, which later roadmap tasks can
cite.

The catalogue should use these classification values exactly: `authoritative`,
`supporting`, `superseded`, and `needs reconciliation`.

The catalogue should use these follow-up labels exactly where applicable:
`update design document`, `merge into PWA design`, `update data model`,
`write ADR`, `update OpenAPI`, `update AsyncAPI`, `roadmap citation fix`, and
`implementation follow-up`.

## External references used during planning

Firecrawl confirmed these current external references for the plan:

- WCAG 2.2 is a W3C Recommendation, and W3C describes its success criteria as
  testable statements: <https://www.w3.org/TR/WCAG22/>.
- Playwright's official accessibility guide recommends
  `@axe-core/playwright` for automated checks and cautions that automated
  checks do not find every accessibility problem:
  <https://playwright.dev/docs/accessibility-testing>.
- LemmaScript appears as a TypeScript verification toolchain under
  `midspiral/LemmaScript`: <https://github.com/midspiral/LemmaScript>.
- External search did not find reliable upstream documentation for the local
  `css-view` command. Use the installed command's help output as the local
  contract during implementation.
