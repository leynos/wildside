# Front-end source authority catalogue

This catalogue records which document owns each Wildside front-end requirement
before implementation work uses `docs/frontend-roadmap.md` as an execution
queue. It is intentionally a source-of-truth map, not a design document. When
documents overlap or disagree, this catalogue names the follow-up that must
settle the design in its proper home.

`docs/v2a-front-end-stack.md` takes precedence where it conflicts with older
Progressive Web Application (PWA) platform guidance. Older PWA material remains
useful when it describes Wildside behaviour that does not conflict with the v2a
stack direction.

## Classification legend

- `authoritative`: owns the requirement for this topic and should be cited by
  implementation tasks.
- `supporting`: supplies background, examples, migration help, or testing
  practice but does not own the requirement.
- `superseded`: contains guidance that should not be followed for this topic
  because a newer authority has replaced it.
- `needs reconciliation`: must be updated or decided before implementation can
  cite one stable authority.

Follow-up labels use these values: `update design document`,
`merge into PWA design`, `update data model`, `write ADR`, `update OpenAPI`,
`update AsyncAPI`, `roadmap citation fix`, and `implementation follow-up`.

## Source inventory

- `docs/v2a-front-end-stack.md` is authoritative for the current
  `frontend-pwa` stack, the target v2a stack, and the precedence rule for stack
  conflicts. It is supporting for entity-localization primitives and testing
  gates.
- `docs/wildside-pwa-design.md` is authoritative for Wildside runtime
  behaviour, routing intent, local-first behaviour, PWA installability, service
  worker policy, caching policy, accessibility expectations, and testing-gate
  intent where it does not conflict with `docs/v2a-front-end-stack.md`.
- `docs/wildside-pwa-data-model.md` is authoritative for entity shapes,
  on-wire schema intent, localization maps, offline bundle models, outbox
  mutation concepts, and backend hexagon mapping.
- `docs/wildside-ux-state-graph-v0.1.json` is authoritative for user
  experience states, transitions, state metadata, persistence annotations, and
  UI surfaces.
- `docs/sitemap.md` is authoritative for planned route paths, navigation
  groups, feature modules, and main journey diagrams where it agrees with the
  state graph.
- `spec/openapi.json` is authoritative for implemented REST wire contracts.
- `spec/asyncapi.yaml` is authoritative for implemented WebSocket and event
  wire contracts.
- `docs/adr-001-websockets-on-actix-ws.md` is authoritative for the accepted
  backend WebSocket adapter decision. It is supporting, not authoritative, for
  front-end event semantics.

## Topic catalogue

### Runtime and build stack

Authority: `docs/v2a-front-end-stack.md` (`authoritative`).

Supporting sources: `docs/react-tailwind-with-bun.md`,
`frontend-pwa/package.json`, and `Makefile` (`supporting`).

Current status: the repository declares Bun, Vite, React, React DOM, TanStack
Query, Tailwind CSS `^3`, DaisyUI `^4`, Zod, clsx, TypeScript, Vitest, and
Orval. The fuller v2a target adds TanStack Router, Tailwind CSS v4, DaisyUI v5,
Radix UI, i18next plus Fluent, MapLibre GL JS, Dexie, Zustand, and XState.

Follow-up: Tailwind CSS v4 and DaisyUI v5 migration remains a later
`implementation follow-up` owned by roadmap items 0.2.4, 0.2.5, and 1.1.x.

### Routing

Authority: `docs/wildside-pwa-design.md` (`authoritative`).

Supporting sources: `docs/v2a-front-end-stack.md`,
`docs/wildside-ux-state-graph-v0.1.json`, and `docs/sitemap.md` (`supporting`).

Current status: the design expects a TanStack Router route tree and accessible
client-side routing. The current `frontend-pwa` package has a single shell and
does not yet declare TanStack Router.

Follow-up: introducing the router and route tree is an
`implementation follow-up` for later roadmap phases. No source-document
contradiction blocks item 0.1.1.

### State management

Authority: `docs/v2a-front-end-stack.md` (`authoritative`).

Supporting sources: `docs/wildside-pwa-design.md` and
`docs/local-first-react.md` (`supporting`).

Current status: current code uses React state, hooks, the theme provider, and
TanStack Query. The v2a target splits responsibility across Zustand for
interactive client state, TanStack Query for server and synchronized domain
state, and XState for explicit multistep workflows.

Follow-up: the long-lived client-state ownership policy should be formalized as
a `write ADR` follow-up before broad feature implementation depends on it.

### Local-first persistence

Authority: `docs/wildside-pwa-design.md` (`authoritative`).

Supporting sources: `docs/wildside-pwa-data-model.md`,
`docs/v2a-front-end-stack.md`, and `docs/local-first-react.md` (`supporting`).

Current status: normal domain data belongs in TanStack Query and should be
persisted to IndexedDB. Heavy assets and mutation queues belong in Dexie or
Cache Storage according to the asset type and operational requirements.

Follow-up: cache versioning, outbox retry semantics, and conflict-resolution
policy need a `write ADR` follow-up if they become shared platform policy
rather than feature-local implementation details.

### PWA installability

Authority: `docs/wildside-pwa-design.md` (`authoritative`).

Supporting source:
`docs/building-accessible-and-responsive-progressive-web-applications.md`
(`supporting`).

Current status: Wildside is intended to be installable with a Web App Manifest,
app-like display mode, icons, `start_url`, and theme colours.

Follow-up: manifest implementation is an `implementation follow-up` for the PWA
hardening workstream. No additional source reconciliation is needed for item
0.1.1.

### Service worker and caching policy

Authority: `docs/wildside-pwa-design.md` (`authoritative`).

Supporting source: `docs/wildside-pwa-data-model.md` (`supporting`).

Current status: the app shell uses cache-first precaching, catalogue reads use
network-first with cached fallback, route generation status uses network-only
or network-first plus WebSocket events, and tile caching differs between normal
browsing and offline bundles.

Follow-up: service-worker update strategy and cache-version governance should
be captured by a `write ADR` follow-up before implementation hardens the
runtime policy.

### Map stack

Authority: `docs/wildside-pwa-design.md` (`authoritative`).

Supporting sources: `docs/v2a-front-end-stack.md`,
`docs/wildside-pwa-data-model.md`, and `docs/sitemap.md` (`supporting`).

Current status: Wildside's target map stack uses MapLibre GL JS, a stable map
canvas per map route view, OpenMapTiles-backed styling, a MapLibre
right-to-left (RTL) text plugin when RTL locales are active, and shared map
state for viewport and overlays.

Follow-up: MapLibre dependency introduction and tile-provider strategy are later
`implementation follow-up` and `write ADR` candidates. Current
`frontend-pwa/package.json` does not declare MapLibre.

### Data model and card model

Authority: `docs/wildside-pwa-data-model.md` (`authoritative`).

Supporting sources: `docs/data-model-driven-card-architecture.md` and
`docs/v2a-front-end-stack.md` (`supporting`).

Current status: entity and value-object shapes cover Explore, Discover,
Customize, Map, Safety, offline downloads, user preferences, notes, progress,
walk sessions, route plans, and card-level projections. The backend must not
ship CSS classes; it provides semantic identifiers that the client maps to
presentation.

Follow-up: richer card-model fixture migration is an
`implementation follow-up`. If data-shape gaps are found while reconciling
contracts, they belong in an `update data model` follow-up.

### Localization and RTL

Authority: `docs/wildside-pwa-design.md` and `docs/wildside-pwa-data-model.md`
(`authoritative` as an authority set).

Supporting sources: `docs/v2a-front-end-stack.md` and
`docs/pure-accessible-and-localizable-react-components.md` (`supporting`).

Current status: UI chrome belongs in translation resources, entity display text
belongs in entity localization maps, unsupported locale tags fall back to
`en-GB`, and RTL behaviour should use CSS logical properties plus MapLibre RTL
text support when map labels are present.

Follow-up: actual i18next, Fluent, supported-locale metadata, and RTL test
harness introduction are later `implementation follow-up` items. Any conflict
between UI-resource and entity-owned strings should be settled through
`merge into PWA design` or `update data model`, depending on which surface owns
the string.

### Accessibility

Authority: `docs/wildside-pwa-design.md` (`authoritative`).

Supporting sources:
`docs/high-velocity-accessibility-first-component-testing.md`,
`docs/pure-accessible-and-localizable-react-components.md`, and
`docs/building-accessible-and-responsive-progressive-web-applications.md`
(`supporting`).

Current status: Wildside targets Web Content Accessibility Guidelines (WCAG)
2.2 Level AA. Requirements include semantic HyperText Markup Language (HTML),
Radix primitives for complex widgets, visible focus, focus-not-obscured
behaviour, skip links, route focus management, and route-change announcements.

Follow-up: executable Playwright, axe, and manual accessibility gates are later
`implementation follow-up` items. The policy source is stable for 0.1.1.

### Styling and tokens

Authority: `docs/v2a-front-end-stack.md` and `docs/wildside-pwa-design.md`
(`authoritative` as an authority set).

Supporting sources: `docs/tailwind-v4-guide.md`, `docs/daisyui-v5-guide.md`,
`docs/semantic-tailwind-with-daisyui-best-practice.md`, and
`docs/enforcing-semantic-tailwind-best-practice.md` (`supporting`).

Current status: current implementation uses Tailwind CSS `^3`, DaisyUI `^4`,
semantic project classes, and generated repository tokens. The target
architecture uses Tailwind CSS v4, DaisyUI v5, Radix state attributes, semantic
HTML, semantic classes, and generated design tokens.

Follow-up: current Tailwind v3 and DaisyUI v4 package state conflicts with the
target v4/v5 platform direction. The repository already treats
`docs/v2a-front-end-stack.md` as the precedence source, and migration belongs to
`implementation follow-up` plus 0.2.4 and 0.2.5.

### REST contracts

Authority: `spec/openapi.json` (`authoritative`).

Supporting sources: `docs/wildside-pwa-data-model.md` and
`docs/wildside-pwa-design.md` (`supporting`).

Current status: the implemented REST spec currently covers login, users,
current-user reads, interest and preference writes, route annotations, route
notes, route progress, and health checks.

Follow-up: the data model describes endpoint families that are absent from the
implemented OpenAPI spec, including catalogue explore snapshots, interest-theme
listing as a catalogue endpoint, route generation creation/status/detail, and
offline bundle management. These gaps need `update OpenAPI` before feature
implementation can cite implemented wire contracts.

### WebSocket and event contracts

Authority: `spec/asyncapi.yaml` (`authoritative`).

Supporting sources: `docs/adr-001-websockets-on-actix-ws.md`,
`docs/wildside-pwa-design.md`, and `docs/wildside-ux-state-graph-v0.1.json`
(`supporting`).

Current status: the implemented AsyncAPI contract describes `/ws` display-name
submission, invalid-display-name replies, and user-created events. ADR 001 owns
the backend adapter choice and confirms that WebSocket handling remains an
inbound adapter.

Follow-up: route-generation progress and offline-bundle progress events are
described by the PWA design and state graph but not by `spec/asyncapi.yaml`.
Those gaps need `update AsyncAPI` before implementation treats them as
implemented event contracts.

### UX state graph

Authority: `docs/wildside-ux-state-graph-v0.1.json` (`authoritative`).

Supporting sources: `docs/wildside-pwa-design.md`,
`docs/wildside-pwa-data-model.md`, and `docs/sitemap.md` (`supporting`).

Current status: the graph owns state identifiers, user-visible state metadata,
transition intent, state-specific local and server state, persistence notes,
and accessibility annotations. It covers runtime, sync, welcome, discover,
explore, customize, wizard, map, saved, offline, safety, completion, and auth
states.

Follow-up: auth and future route states exist in the graph but are not
represented as current sitemap routes. This needs `roadmap citation fix` or
`merge into PWA design`, depending on whether they remain future states or
become planned route work.

### Sitemap routes

Authority: `docs/sitemap.md` (`authoritative`).

Supporting sources: `docs/wildside-ux-state-graph-v0.1.json` and
`docs/wildside-pwa-design.md` (`supporting`).

Current status: the sitemap owns planned route paths, bottom navigation groups,
nested map routes, feature modules, and high-level user flows.

Follow-up: route names and future auth states should be reconciled against the
state graph through `roadmap citation fix` or `merge into PWA design` before
later roadmap phases use them as implementation tickets.

### Testing and validation

Authority: `docs/wildside-pwa-design.md` and
`docs/high-velocity-accessibility-first-component-testing.md` (`authoritative`
as an authority set).

Supporting sources: `docs/v2a-front-end-stack.md`,
`docs/wildside-testing-guide.md`, `docs/rstest-bdd-users-guide.md`, `Makefile`,
and `frontend-pwa/package.json` (`supporting`).

Current status: executable repository gates are `make check-fmt`, `make lint`,
`make test`, `make markdownlint`, and `make nixie`. Current front-end unit
tests use Vitest. Target-state front-end validation includes component tests,
accessibility tests, Playwright browser checks, keyboard flows, localization
regression tests, Gherkin behavioural tests where applicable, property tests
where invariants are introduced, and proof tooling where axioms or contractual
business logic require it.

Follow-up: importing the richer v2a lint, Playwright, axe, Gherkin,
`fast-check`, and proof gates is an `implementation follow-up` for 0.2.4 and
later implementation phases.

### Semantic linting

Authority: `docs/enforcing-semantic-tailwind-best-practice.md`
(`authoritative`).

Supporting sources: `docs/semantic-tailwind-with-daisyui-best-practice.md` and
`docs/v2a-front-end-stack.md` (`supporting`).

Current status: the semantic lint policy covers GritQL, Semgrep, Stylelint,
Biome integration, semantic landmarks, heading structure, daisyUI component
class misuse, state-slot classes, class-list length, and raw colour rules.

Follow-up: the rules from the v2a mockup need `implementation follow-up` import
work under roadmap item 0.2.4 before they become executable local gates.

### Property tests

Authority: `docs/wildside-pwa-design.md` (`authoritative` for when validation
is required).

Supporting sources: `docs/v2a-front-end-stack.md` and
`docs/high-velocity-accessibility-first-component-testing.md` (`supporting`).

Current status: item 0.1.1 is documentation-only and introduces no invariant
over inputs, states, orderings, or transitions. Property tests are therefore
not applicable to this catalogue.

Follow-up: when later phases introduce invariants, add `fast-check` property
tests as an `implementation follow-up` in the same feature slice.

### Proof obligations

Authority: `docs/wildside-pwa-design.md` (`authoritative` for when proof is
required).

Supporting sources: `docs/v2a-front-end-stack.md` and the LemmaScript upstream
project (`supporting`).

Current status: item 0.1.1 introduces no axiom or contractual business logic.
An exhaustive proof is therefore not applicable to this catalogue.

Follow-up: if later phases introduce business axioms, protocol invariants, or
contractual state-machine properties, add a substantive proof as an
`implementation follow-up` in that feature slice.

### Documentation ownership

Authority: `docs/documentation-style-guide.md` (`authoritative`).

Supporting sources: `AGENTS.md`, `docs/contents.md`, and this catalogue
(`supporting`).

Current status: design decisions belong in design documents or ADRs,
implementation queues belong in roadmaps, and source-authority classification
belongs in this catalogue. Markdown uses en-GB-oxendict spelling and the
project Markdown rules.

Follow-up: if later reconciliation turns catalogue follow-ups into accepted
policy, update the owning design document or write an ADR. Do not leave policy
only in `docs/frontend-roadmap.md`.

### Hexagonal boundary ownership

Authority: `docs/wildside-pwa-data-model.md` (`authoritative`).

Supporting sources: `docs/wildside-backend-architecture.md`,
`docs/adr-001-websockets-on-actix-ws.md`, and the `hexagonal-architecture` skill
(`supporting`).

Current status: domain types and ports remain framework-independent, adapters
translate at boundaries, and front-end code consumes backend contracts through
OpenAPI, AsyncAPI, generated clients, or documented ports. The WebSocket ADR
confirms that backend event handling remains an inbound adapter concern.

Follow-up: any front-end implementation that would depend directly on backend
adapter internals must stop and create an `update design document` or
`write ADR` follow-up instead.

## Reconciliation follow-ups

- `update OpenAPI`: add or explicitly defer catalogue explore snapshots,
  interest-theme listing as a catalogue contract, route generation
  creation/status/detail, route annotations parity, and offline bundle
  management before front-end feature work treats them as implemented REST
  contracts.
- `update AsyncAPI`: add or explicitly defer route-generation progress,
  offline-bundle progress, and other progress events described by the PWA
  design and state graph before front-end feature work treats them as
  implemented WebSocket contracts.
- `write ADR`: settle durable front-end platform policy for client-state
  ownership, service-worker update strategy, cache versioning, outbox retry
  semantics, conflict handling, and map tile provider strategy.
- `merge into PWA design`: reconcile auth and future route states between the
  state graph, sitemap, and staged PWA design.
- `roadmap citation fix`: once the design and contract owners are reconciled,
  replace later roadmap prose that reads as policy with citations to the
  authoritative source.
- `implementation follow-up`: import target-state validation gates, semantic
  linting, Playwright accessibility checks, localization checks, Gherkin
  behavioural tests, `fast-check` property tests, and substantive proof tooling
  only in the feature slices that require them.

## Relevant skills and tooling

- Use the `leta` skill for code navigation before any code or symbol-level
  refactor.
- Use the `rust-router` skill before Rust implementation work, then load the
  smallest relevant Rust follow-on skill.
- Use the `hexagonal-architecture` skill when changing or reviewing boundaries
  between domain policy, ports, and adapters.
- Use the `execplans` skill for non-trivial implementation plans and keep the
  plan's living sections current.
- Use Firecrawl when current external information is needed for open-source
  tooling, common protocols, specifications, or prior art.

## Validation note for item 0.1.1

This catalogue changes documentation only. It introduces no executable
front-end surface, runtime strings, card model fixture data, APIs, persistence,
or CSS. Playwright and `css-view` therefore have no rendered UI to validate for
this item. They remain required gates for later executable front-end work, with
no errors, failures, or accessibility violations permitted when applicable.
