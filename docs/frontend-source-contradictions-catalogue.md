# Front-end source contradictions catalogue

This catalogue extends the
[front-end source authority catalogue](frontend-source-authority-catalogue.md)
with concrete contradictions, duplicated requirements, and contract gaps found
while implementing roadmap item 0.1.2. It is a triage aid, not a design
authority. Findings remain open until the owning design document, data model,
Architecture Decision Record (ADR), OpenAPI specification, AsyncAPI
specification, or roadmap citation is updated.

No finding may be resolved by adding requirement prose only to
`docs/frontend-roadmap.md`. The roadmap may cite a resolved source; it must not
become the source that settles product policy, schema shape, platform policy,
wire contracts, or user experience rules.

Closing pull requests must update the affected row's `status` field to
`resolved by PR #NNNN`. If a later change makes the finding irrelevant, use
`superseded`. If the row is found to be an audit error, use `withdrawn`.

## Label set and contract-gap ownership

Every finding uses exactly one of these labels:

- `update design document`
- `merge into Progressive Web App design`
- `update data model`
- `write Architecture Decision Record`
- `roadmap citation fix`

Contract gaps are routed through the ownership tree from the ExecPlan:

- Missing or wrong fields inside an existing operation or event shape are
  labelled `update data model`.
- Missing endpoint or event surfaces are labelled `update design document`, with
  `update OpenAPI` or `update AsyncAPI` as the sub-resolution.
- Wire-level encoding details where intent already agrees are labelled
  `update design document`, with the relevant contract sub-resolution.
- Cross-cutting platform invariants are labelled
  `write Architecture Decision Record`.

## Canonical row schema

Rows in the findings table are a rendered form of this schema:

```yaml
- id: FIND-NNNN
  topic: <short topic name>
  status: open
  severity: blocking
  label: update design document
  sub_resolution: update OpenAPI
  perishability: post-v2a
  sources:
    - path: docs/...
      anchor: §<section> or L<line>-L<line>
    - path: spec/...
      anchor: <jsonpointer> or L<line>-L<line>
  claims:
    - source: a
      bcp14: MUST
      summary: <one short sentence>
      evidence: "<short quoted excerpt, <=25 words>"
    - source: b
      bcp14: SHOULD
      summary: ...
      evidence: ...
  rationale: <one or two sentences explaining the label choice>
  ownership_note: <one sentence, cites a hexagonal invariant when relevant>
  authority_catalogue_topic: <topic heading in docs/frontend-source-authority-catalogue.md>
  siblings: [FIND-NNNN, ...]
```

## Findings

| ID        | Topic                                              | Status | Severity  | Label                                 | Sub-resolution  | Perishability | Sources                                                                                                                                                 | Claim A                                                                                                                                                                | Claim B                                                                                                                                              | Rationale                                                                                                                                                                         | Ownership note                                                                                             | Authority topic                   |
| --------- | -------------------------------------------------- | ------ | --------- | ------------------------------------- | --------------- | ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | --------------------------------- |
| FIND-0001 | Image alt-text localization shape                  | open   | important | update data model                     |                 | post-v2a      | `docs/wildside-pwa-data-model.md` L129-L138; `docs/v2a-front-end-stack.md` L267-L283                                                                    | A `SHOULD`: `ImageAsset` uses `alt: string`. Evidence: "`readonly alt: string;`"                                                                                       | B `SHOULD`: v2a `ImageAsset` carries localized alt text. Evidence: "`ImageAsset` — a reference ... together with its `LocalizedAltText`."            | The same shared primitive has incompatible accessibility and localization shape. The data model owns entity field shape.                                                          | Domain purity is preserved by storing localized semantic content, not presentation behaviour.              | Data model and card model         |
| FIND-0002 | Presentation class fields in card schemas          | open   | important | update data model                     |                 | post-v2a      | `docs/wildside-pwa-data-model.md` L179-L181; `docs/data-model-driven-card-architecture.md` L107-L143                                                    | A `MUST NOT`: backend-owned entities must not store Tailwind class strings. Evidence: "must not store Tailwind class strings."                                         | B `SHOULD`: card schemas still include `gradientClass` and `accentClass`. Evidence: "`gradientClass: string`"                                        | The card-architecture support document leaks presentation classes into schema examples. The data model should settle semantic token identifiers versus presentation classes.      | Hexagonal domain purity requires semantic identifiers at the contract boundary; adapters map them to CSS.  | Hexagonal boundary ownership      |
| FIND-0003 | Catalogue and descriptor REST surfaces             | open   | blocking  | update design document                | update OpenAPI  | post-v2a      | `docs/wildside-pwa-data-model.md` L297-L314 and L622-L628; `spec/openapi.json` L412-L472 and L1125-L1170                                                | A `SHOULD`: the PWA needs an Explore snapshot and interest-theme endpoint. Evidence: "`GET /api/v1/catalogue/explore`"                                                 | B `informative`: OpenAPI exposes login, route subresources, user endpoints, and health probes, but no catalogue paths. Evidence: "`/api/v1/login`"   | This is a missing endpoint surface, not a field mismatch. The design document should own the surface intent before OpenAPI is updated.                                            | The frontend consumes catalogue data through an HTTP port, not backend repositories directly.              | REST contracts                    |
| FIND-0004 | Route-generation REST lifecycle                    | open   | blocking  | update design document                | update OpenAPI  | post-v2a      | `docs/wildside-pwa-data-model.md` L630-L632; `docs/wildside-ux-state-graph-v0.1.json` L1317-L1353 and L5510-L5592; `spec/openapi.json` L472-L710        | A `SHOULD`: route generation uses `POST /api/v1/routes` and request-status polling. Evidence: "`POST /api/v1/routes`"                                                  | B `informative`: OpenAPI only exposes route annotations, notes, and progress subresources. Evidence: "`/api/v1/routes/{route_id}/annotations`"       | The route-generation endpoint family is absent from the implemented contract while multiple source documents depend on it.                                                        | The generation workflow should enter the backend through documented driving adapters.                      | REST contracts                    |
| FIND-0005 | Offline-bundle REST lifecycle                      | open   | important | update design document                | update OpenAPI  | post-v2a      | `docs/wildside-pwa-data-model.md` L447-L472 and L636-L638; `spec/openapi.json` L412-L472 and L1125-L1170                                                | A `SHOULD`: offline bundles are first-class manifests with GET, POST, and DELETE endpoints. Evidence: "`GET /api/v1/offline/bundles`"                                  | B `informative`: OpenAPI contains no offline-bundle path family. Evidence: "`/health/ready`"                                                         | This is a missing endpoint surface. The PWA design should clarify whether bundle management is in the first OpenAPI expansion.                                                    | Tile bytes remain adapter storage; the contract should expose bundle manifests only.                       | REST contracts                    |
| FIND-0006 | Interests optimistic concurrency field             | open   | important | update data model                     |                 | post-v2a      | `docs/wildside-pwa-data-model.md` L384-L401; `spec/openapi.json` L4-L19 and L915-L979                                                                   | A `MUST`: after the first write, interests updates send `expectedRevision` and handle `409`. Evidence: "must send `expectedRevision`"                                  | B `informative`: `InterestsRequest` only requires `interestThemeIds`. Evidence: "`required`: [`interestThemeIds`]"                                   | This is a missing field inside an existing operation shape, so the data model owns the correction before OpenAPI follows.                                                         | The field protects domain aggregate concurrency across the HTTP adapter boundary.                          | Data model and card model         |
| FIND-0007 | Idempotency-key scope and TTL                      | open   | blocking  | write Architecture Decision Record    |                 | post-v2a      | `docs/wildside-pwa-data-model.md` L583-L598; `spec/openapi.json` L550-L576, L671-L698 and L1035-L1051                                                   | A `MUST`: all outbox-backed mutation endpoints follow the idempotency contract. Evidence: "All outbox-backed mutation endpoints follow this contract shape."           | B `informative`: OpenAPI advertises optional `Idempotency-Key` on only some writes. Evidence: "`required`: false"                                    | The scope, requiredness, replay semantics, and TTL affect client, backend, and contract behaviour. That makes the policy ADR-shaped.                                              | A cross-cutting invariant should be settled once, then reflected in ports and adapters.                    | Local-first persistence           |
| FIND-0008 | Route-generation WebSocket events                  | open   | blocking  | update design document                | update AsyncAPI | post-v2a      | `docs/wildside-pwa-design.md` L281-L294; `docs/wildside-ux-state-graph-v0.1.json` L1381-L1385 and L5607-L5698; `spec/asyncapi.yaml` L12-L64             | A `SHOULD`: route-generation progress can arrive through WebSocket events. Evidence: "route generation status"                                                         | B `informative`: AsyncAPI only lists display-name and user-created messages on `/ws`. Evidence: "`displayNameRequest`"                               | This is a missing channel/event surface. The design document should own progress-event intent before AsyncAPI changes.                                                            | WebSocket framing remains an inbound adapter contract; the frontend should consume documented events only. | WebSocket and event contracts     |
| FIND-0009 | Offline-bundle progress events                     | open   | important | update design document                | update AsyncAPI | post-v2a      | `docs/frontend-source-authority-catalogue.md` L267-L270; `docs/wildside-pwa-data-model.md` L447-L472; `spec/asyncapi.yaml` L12-L64                      | A `SHOULD`: offline-bundle progress events are expected follow-up contract work. Evidence: "offline-bundle progress events"                                            | B `informative`: AsyncAPI only declares `/ws` user/display-name messages. Evidence: "`userCreated`"                                                  | Offline bundle lifecycle has manifest status/progress but no implemented event contract. The design document should decide whether progress is WebSocket, polling, or local-only. | The bundle manifest is domain data; progress transport is adapter policy.                                  | WebSocket and event contracts     |
| FIND-0010 | Auth route and current sitemap phase               | open   | minor     | merge into Progressive Web App design |                 | post-v2a      | `docs/wildside-pwa-design.md` L120-L139 and L423-L432; `docs/wildside-ux-state-graph-v0.1.json` L2846-L2965 and L8295-L8305; `docs/sitemap.md` L5-L23   | A `MAY`: the PWA design has an `auth/` feature and Stage 5 auth/account handling. Evidence: "`safety/`, `auth/`"                                                       | B `informative`: the sitemap route table has no visible auth route. Evidence: "`/safety-accessibility`"                                              | The graph intentionally carries future auth states, but the route authority needs a clear phase boundary, so implementation tasks cite it consistently.                           | Authentication UI should be planned as a route slice, not inferred from backend login endpoints.           | UX state graph                    |
| FIND-0011 | Service-worker update activation policy            | open   | important | write Architecture Decision Record    |                 | post-v2a      | `docs/building-accessible-and-responsive-progressive-web-applications.md` L434-L460; `docs/wildside-pwa-design.md` L367-L373                            | A `MAY`: the general PWA guide describes immediate activation via `skipWaiting()` and `clients.claim()`. Evidence: "provide a pattern for forcing an immediate update" | B `SHOULD`: Wildside should wait by default and only force activation with tested visible UX. Evidence: "Default to waiting for the next navigation" | The sources are not irreconcilable, but the runtime invariant is cross-cutting enough to need an ADR before service-worker work hardens.                                          | Service-worker update semantics affect runtime, cache, UI, and deployment adapters.                        | Service worker and caching policy |
| FIND-0012 | Current styling stack versus v2a semantic guidance | open   | minor     | roadmap citation fix                  |                 | pre-v2a       | `docs/v2a-front-end-stack.md` L17-L24 and L126-L143; `docs/semantic-tailwind-with-daisyui-best-practice.md` L1-L23; `frontend-pwa/package.json` L20-L39 | A `informative`: current dependencies are Tailwind v3 and DaisyUI v4. Evidence: "`tailwindcss`: `^3`"                                                                  | B `SHOULD`: semantic guidance is written for Tailwind v4 and DaisyUI v5. Evidence: "Tailwind CSS v4 utilities, daisyUI v5 roles"                     | This is known stack drift, not a per-feature contradiction. Later roadmap citations should consistently distinguish current package state from target v2a guidance.               | Styling migration should not leak target-only assumptions into domain or contract tasks.                   | Styling and tokens                |
| FIND-0013 | Bottom-navigation terminology                      | open   | minor     | merge into Progressive Web App design |                 | post-v2a      | `docs/sitemap.md` L24-L33 and L106-L119; `docs/wildside-ux-state-graph-v0.1.json` L3527-L3611 and L5899-L6070                                           | A `SHOULD`: the primary bottom nav labels `/explore` as Discover and `/customize` as Routes. Evidence: "**Discover** (`/explore`)"                                     | B `informative`: graph transitions also use labels including Discover, Routes, and Explore for related surfaces. Evidence: "`label`: `Discover`"     | The terms are probably intentional product copy, but Discover is also a route name and Explore is also a surface. The PWA design should own the user-facing taxonomy.             | Navigation labels are UI policy, not backend contract shape.                                               | Sitemap routes                    |

## Duplicate but consistent requirements

These candidates were reviewed and intentionally not treated as contradictions.

| ID       | Topic                        | Status | Severity | Label                | Perishability | Sources                                                                                                                                    | Resolution                                                                                                                                                             |
| -------- | ---------------------------- | ------ | -------- | -------------------- | ------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| DUP-0001 | Offline tile storage options | open   | minor    | roadmap citation fix | post-v2a      | `docs/wildside-pwa-design.md` L326-L338; `docs/wildside-pwa-data-model.md` L475-L490                                                       | Both sources intentionally keep Cache Storage and a Dexie tile table compatible. Future tasks should cite the design for policy and the data model for manifest shape. |
| DUP-0002 | Outbox idempotent replay     | open   | minor    | roadmap citation fix | post-v2a      | `docs/wildside-pwa-design.md` L313-L324; `docs/wildside-pwa-data-model.md` L583-L598; `docs/wildside-ux-state-graph-v0.1.json` L8528-L8539 | The sources repeat the same replay intent. FIND-0007 tracks the unresolved policy and contract scope rather than duplicating this row per mutation.                    |

## Topic cross-reference

| Catalogue topic                | Authority catalogue topic         | Findings                                   |
| ------------------------------ | --------------------------------- | ------------------------------------------ |
| Image and card schema shape    | Data model and card model         | FIND-0001, FIND-0002                       |
| REST endpoint families         | REST contracts                    | FIND-0003, FIND-0004, FIND-0005, FIND-0006 |
| Idempotency and offline writes | Local-first persistence           | FIND-0007, DUP-0002                        |
| WebSocket event families       | WebSocket and event contracts     | FIND-0008, FIND-0009                       |
| Auth and state-graph phase     | UX state graph                    | FIND-0010                                  |
| Service-worker policy          | Service worker and caching policy | FIND-0011                                  |
| Styling stack migration        | Styling and tokens                | FIND-0012                                  |
| Navigation taxonomy            | Sitemap routes                    | FIND-0013                                  |
| Offline tile storage           | Local-first persistence           | DUP-0001                                   |

## Coverage matrix

| Source                                                                    | Role in audit | Audited range | Notes                                                                                                  |
| ------------------------------------------------------------------------- | ------------- | ------------- | ------------------------------------------------------------------------------------------------------ |
| `docs/v2a-front-end-stack.md`                                             | Primary       | L1-L369       | Stack, localization, card architecture, map stack, and validation target.                              |
| `docs/building-accessible-and-responsive-progressive-web-applications.md` | Primary       | L1-L1338      | PWA, service-worker, caching, responsive, accessibility, and offline guidance.                         |
| `docs/semantic-tailwind-with-daisyui-best-practice.md`                    | Primary       | L1-L155       | Tailwind v4, DaisyUI v5, semantic HTML, Radix state, and token guidance.                               |
| `docs/wildside-pwa-design.md`                                             | Primary       | L1-L451       | PWA architecture, module layout, data access, offline policy, accessibility, and stages.               |
| `docs/wildside-pwa-data-model.md`                                         | Primary       | L1-L874       | Entity schemas, route generation, user state, offline bundles, endpoints, and backend hexagon mapping. |
| `docs/wildside-ux-state-graph-v0.1.json`                                  | Primary       | L1-L8609      | State identifiers, transitions, API mentions, assumptions, and test assertions.                        |
| `docs/sitemap.md`                                                         | Primary       | L1-L141       | Route table, bottom navigation, state diagram, feature modules, and user flows.                        |
| `spec/openapi.json`                                                       | Primary       | L1-L1197      | REST operation inventory and schema-field checks.                                                      |
| `spec/asyncapi.yaml`                                                      | Primary       | L1-L198       | WebSocket channel and message inventory.                                                               |
| `docs/frontend-source-authority-catalogue.md`                             | Supporting    | L1-L447       | Authority topics, prior follow-ups, and reconciliation labels.                                         |
| `docs/data-model-driven-card-architecture.md`                             | Supporting    | L1-L440       | Card schema and presentation-field cross-checks.                                                       |
| `frontend-pwa/package.json`                                               | Supporting    | L1-L40        | Current package-state check for stack drift.                                                           |

## Audit artefacts

The reproducible UX graph walker is
`scripts/audit-ux-state-graph.mjs`. The Stage B.1 scratch pass wrote logs under
`/tmp` using the template
`audit-$(get-project)-$(git branch --show-current)` and these suffixes:

- `.out.openapi-ops`
- `.out.asyncapi-channels`
- `.out.uxstates`
- `.out.crossref`

The UX walker reported 74 states and 18 orphan markers. Stage B.3 treated the
orphan list as a candidate generator only. Terminal, transient, error, and
future states were not promoted unless they exposed a cross-document
contradiction.

## Validation note

This catalogue introduces no runtime strings, frontend components, executable
UI behaviour, schemas consumed by the app, OpenAPI contract edits, AsyncAPI
contract edits, generated artefacts, package manifests, or lockfile changes.
Playwright and `css-view` therefore have no rendered UI surface to validate for
item 0.1.2. They remain required gates for later executable front-end work.

The applicable gates for this documentation and audit-script change are:

- `make check-fmt`
- `make lint`
- `make markdownlint`
- `make nixie`
- `make test`
- `coderabbit review --agent`

## Relevant skills and tooling

- `leta` was loaded for code-aware navigation where symbol-level checks are
  needed.
- `rust-router` was loaded because the repository is Rust-based, though this
  item changes no Rust source.
- `hexagonal-architecture` informed ownership notes for domain, port, and
  adapter boundaries.
- `execplans` governs the living implementation plan.
- `Firecrawl` informed the earlier plan drafting; no new external source is
  required by this catalogue.
