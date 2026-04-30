# Wildside front-end roadmap

This roadmap translates the Wildside front-end design documents, API contracts,
user experience state graph, and v2a mockup into an outcome-oriented delivery
sequence for `frontend-pwa/`. It does not promise dates. Each phase carries one
testable idea at the Goals, Ideas, Steps, Tasks (GIST) level; steps validate
that idea through sequenced workstreams, and tasks are review-sized execution
units with explicit acceptance criteria.

The primary source material is `docs/wildside-pwa-design.md`,
`docs/wildside-pwa-data-model.md`, `docs/wildside-ux-state-graph-v0.1.json`,
`docs/sitemap.md`, `spec/openapi.json`, `spec/asyncapi.yaml`, and the v2a mockup
in `../wildside-mockup-v2a`. The styling, testing, localization, and Progressive
Web Application (PWA) constraints come from the supporting
documents under `docs/`.

The roadmap is a catalogue of correctly sized implementation tasks. It signposts
authoritative design documents, Architecture Decision Records, schemas, and API
contracts through inline citations, but it is not the principal source of
product policy, schema shape, platform, or user experience requirements.

## 0. Source-of-truth reconciliation before implementation

Idea: if the front-end sources of truth are reconciled before foundational build
work begins, implementation tasks can cite stable design authority instead of
accumulating decisions inside the roadmap.

This phase catalogues the documentation reconciliation work needed before the
roadmap is used as an implementation queue. It keeps design decisions in design
documents or Architecture Decision Records (ADRs),
resolves known inconsistencies where older Progressive Web App material differs
from the v2a stack direction, and then refreshes roadmap citations so later
phases remain task-focused.

### 0.1. Catalogue authority, overlaps, and contradictions

This step answers which document owns each front-end requirement and where
contradictions need a design-document or Architecture Decision Record update.
The outcome informs the source documents that phases 1-5 should cite.

- [ ] 0.1.1. Build a front-end source authority catalogue.
  - Classify `docs/v2a-front-end-stack.md`, `docs/wildside-pwa-design.md`,
    `docs/wildside-pwa-data-model.md`, `docs/wildside-ux-state-graph-v0.1.json`,
    `docs/sitemap.md`, `spec/openapi.json`, `spec/asyncapi.yaml`, and relevant
    Architecture Decision Records as authoritative, supporting, superseded, or
    needing reconciliation per topic.
  - Record that `docs/v2a-front-end-stack.md` takes precedence where it
    conflicts with older Progressive Web App platform guidance.
  - Success: every platform, data, user experience-state, API, and styling topic
    referenced by this roadmap has one named authoritative source or a named
    reconciliation follow-up.
- [ ] 0.1.2. Catalogue contradictions and duplicated requirements.
  - Requires 0.1.1.
  - Review the v2a stack, accessible Progressive Web App guide, semantic
    Tailwind/DaisyUI guide, Wildside Progressive Web App design, Wildside
    Progressive Web App data model, user experience state graph, sitemap, and
    API specs for conflicting or duplicated requirements.
  - Label each finding as "update design document", "merge into Progressive Web
    App design", "update data model", "write Architecture Decision Record", or
    "roadmap citation fix".
  - Success: no finding is resolved by adding requirement prose only to this
    roadmap.

### 0.2. Move decisions into the right design authority

This step answers which inconsistencies can be settled in existing design
documents and which require a separate Architecture Decision Record. The outcome
informs the concrete implementation tasks in phases 1-4.

- [ ] 0.2.1. Reconcile Progressive Web App platform guidance under the v2a stack
      direction.
  - Requires 0.1.2.
  - Update, merge, or supersede `docs/wildside-pwa-design.md` so Progressive Web
    App platform requirements align with `docs/v2a-front-end-stack.md` on stack
    ownership, local-first persistence, MapLibre, localization, semantic
    styling, and test tooling.
  - See `docs/v2a-front-end-stack.md` §§Overview, State management, Map stack,
    and Testing and verification stack; and `docs/wildside-pwa-design.md`.
  - Success: later roadmap tasks can cite one current Progressive Web App design
    source for platform implementation requirements.
- [ ] 0.2.2. Formalize substantive platform policy decisions as Architecture
      Decision Records.
  - Requires 0.1.2.
  - Write Architecture Decision Records, rather than roadmap prose, for
    decisions that affect long-lived platform policy, such as client-state
    ownership, service-worker/deployment scope, cache and persistence
    versioning, outbox retry semantics, map tile provider strategy, or test-gate
    policy.
  - See `docs/v2a-front-end-stack.md`,
    `docs/building-accessible-and-responsive-progressive-web-applications.md`,
    and `docs/wildside-pwa-design.md`.
  - Success: each substantive policy choice is either documented in a design
    document section small enough to own it clearly or linked from a dedicated
    Architecture Decision Record.
- [ ] 0.2.3. Reconcile schema and contract inconsistencies in source documents.
  - Requires 0.1.2.
  - Update `docs/wildside-pwa-data-model.md`, OpenAPI, AsyncAPI, or their owning
    design notes for schema-shape and contract gaps before implementation tasks
    depend on them.
  - Include known reconciliation areas such as localized media alt text,
    narrative-snippet cache state, feedback/reporting contracts, offline
    mutation types, and route-plan persistence metadata.
  - Success: implementation tasks cite schemas and contracts that already
    contain the relevant field shapes, event shapes, and mutation semantics.
- [ ] 0.2.4. Import and document the v2a front-end lint gates.
  - Requires 0.1.2.
  - Import the localization, accessibility, semantic CSS, testing-selector, and
    architectural lint rules from
    <https://github.com/leynos/wildside-mockup-v2a>.
  - Cover the mockup sources `package.json`, `biome.jsonc`,
    `vitest.a11y.config.ts`, `playwright.config.ts`,
    `tests/setup-vitest-a11y.ts`, `scripts/check-fluent-vars.ts`,
    `scripts/check-classlist-length.ts`,
    `scripts/find-near-duplicate-classes.ts`, `tools/grit/`,
    `tools/semantic-lint.config.json`, `tools/semgrep-semantic.yml`, and
    `tools/stylelint.config.cjs`.
  - Document which imported checks belong to Makefile gates, which remain
    advisory, and which design or Architecture Decision Record source owns each
    policy.
  - Success: the repository can trace every imported v2a lint to source,
    documentation, and an executable or explicitly deferred gate.
- [ ] 0.2.5. Import the current v2a tokens and design-system document.
  - Requires 0.1.2.
  - Import the most recent design-token set and design-system documentation from
    <https://github.com/leynos/wildside-mockup-v2a>.
  - Cover the mockup sources `tokens/src/tokens.json`,
    `tokens/src/themes/light.json`, `tokens/src/themes/dark.json`,
    `tokens/build/style-dictionary.js`, `tokens/build/validate-contrast.js`,
    `tokens/src/utils/`, `tailwind.config.cjs`, `postcss.config.cjs`,
    `src/index.css`, and `docs/wildside-mockup-design.md`.
  - Integrate the imported token source with the repository-owned
    `packages/tokens/` pipeline rather than committing generated artefacts from
    the mockup.
  - Success: the imported token source, design-system documentation, and local
    token build path are traceable to one another, and local token generation
    produces the CSS variables, Tailwind theme fragments, DaisyUI roles, and
    contrast checks required by the v2a design system.

### 0.3. Refresh the roadmap after authority is settled

This step answers whether the roadmap remains an implementation catalogue after
the design sources are cleaned up. The outcome unblocks phase 1 without using
the roadmap as a hidden design document.

- [ ] 0.3.1. Replace decision prose in later phases with citations.
  - Requires steps 0.1-0.2.
  - Audit phases 1-5 for wording that makes the roadmap the primary source of a
    requirement, policy, schema shape, or user experience rule.
  - Convert those passages into implementation tasks that cite the authoritative
    design document, Architecture Decision Record, API spec, or user experience
    graph section.
  - Success: each task still has measurable acceptance criteria, but the
    underlying requirement lives in a cited source document.
- [ ] 0.3.2. Recheck dependencies after reconciliation.
  - Requires 0.3.1.
  - Update task dependencies where Architecture Decision Records,
    design-document merges, schema updates, or contract updates now gate
    implementation.
  - Success: no phase 1-5 task depends on an unresolved design contradiction or
    undocumented policy decision.

## 1. Foundational front-end contracts and build spine

Idea: if the front-end settles its runtime stack, route-state contract,
schema-validation boundary, and accessibility-first quality gates before feature
work expands, later slices can migrate from fixtures to backend data without
repeatedly reshaping app structure.

This phase turns the current minimal Progressive Web App into the
production-ready skeleton that feature slices can reuse. It deliberately
resolves version alignment, provider boundaries, route metadata, generated API
shape, and verification rules first because every user-facing slice depends on
them.

### 1.1. Ratify the target stack and migration boundary

This step answers what `frontend-pwa/` is allowed to become before feature
delivery starts. Its outcome informs dependency changes, token plumbing, route
layout, and which mockup practices should be copied rather than re-invented. See
`docs/v2a-front-end-stack.md` §§Runtime and build toolchain, Styling and design
system, and What is not currently declared; `docs/wildside-pwa-design.md`
§§Styling, theming, and tokens; and
`../wildside-mockup-v2a/docs/wildside-mockup-design.md` §§Goals and Migration
workflow.

- [ ] 1.1.1. Record the front-end stack alignment decision in `docs/`.
  - Decide whether this repository upgrades directly to Tailwind CSS v4, DaisyUI
    v5, TanStack Router, Radix UI, i18next plus Fluent, Dexie, and the v2a token
    pipeline, or lands a compatibility bridge first.
  - See `docs/v2a-front-end-stack.md` §§Overview and What is not currently
    declared in the checked-in mockup; `docs/tailwind-v3-v4-migration-guide.md`
    §§Core Architecture & Performance and Key Migration Considerations; and
    `docs/daisyui-v5-guide.md` §§daisyUI 5 install notes and usage rules.
  - Success: one accepted documentation update names the target versions, the
    transition policy for Tailwind v3/DaisyUI v4 code, and the packages that
    must not be introduced.
- [ ] 1.1.2. Normalize package versions and script entry points for the chosen
      stack.
  - Requires 1.1.1.
  - Align `react`, `react-dom`, Vite, Tailwind, DaisyUI, TanStack packages, and
    generated-token scripts with the target decision.
  - See `docs/react-tailwind-with-bun.md` §§Run the dev server and Build for
    production; `docs/v2a-front-end-stack.md` §§Runtime and build toolchain; and
    `docs/tailwind-v4-guide.md` §§Installation & Setup and Framework
    Integration.
  - Success: `make check-fmt`, `make lint`, and `make test` invoke the same
    front-end entry points that developers use locally.
- [ ] 1.1.3. Port the mockup token pipeline into the repository-owned token
      package.
  - Requires 1.1.2 and 0.2.5.
  - Generate runtime CSS custom properties, Tailwind theme fragments, and
    DaisyUI theme roles from `packages/tokens/` rather than hand-maintained
    colour constants.
  - See `docs/v2a-front-end-stack.md` §§Design tokens and Theme handling;
    `docs/semantic-tailwind-with-daisyui-best-practice.md` §11; and
    `../wildside-mockup-v2a/docs/wildside-mockup-design.md` §§Design token
    strategy and Tailwind and DaisyUI integration.
  - Success: day and night themes expose DaisyUI role variables with documented
    contrast checks and no generated artefacts committed.

### 1.2. Establish app shell, providers, and route-state metadata

This step answers whether the route tree can represent the sitemap and the user
experience graph before screens become complex. The outcome informs focus
management, code-splitting, route guards, and offline restore behaviour. See
`docs/sitemap.md` §§Route Structure and User Flows;
`docs/wildside-ux-state-graph-v0.1.json`; and `docs/wildside-pwa-design.md`
§§Frontend module layout and Accessible client-side routing.

- [ ] 1.2.1. Replace the single `App` view with a feature-first application
      shell.
  - Requires 1.1.2.
  - Create `providers/`, `layout/`, `routes/`, `features/`, `lib/`, and `data/`
    boundaries under `frontend-pwa/src/app/`.
  - See `docs/wildside-pwa-design.md` §Frontend module layout and
    `../wildside-mockup-v2a/docs/wildside-mockup-design.md` §Component
    architecture direction.
  - Success: route modules can mount without importing backend fetchers or
    shared layout internals directly.
- [ ] 1.2.2. Implement the TanStack Router route tree from the sitemap and user
      experience state graph.
  - Requires 1.2.1.
  - Cover `/welcome`, `/discover`, `/explore`, `/customize`, `/wizard/*`,
    `/map/*`, `/saved`, `/walk-complete`, `/offline`, and
    `/safety-accessibility`.
  - See `docs/sitemap.md` §Route Structure and
    `docs/wildside-ux-state-graph-v0.1.json` `routeIndex` and `coverageMatrix`.
  - Success: every route in the sitemap resolves, unknown routes render an
    accessible fallback, and root redirects to `/welcome`.
- [ ] 1.2.3. Add route metadata for title, main landmark focus, live
      announcements, and navigation groups.
  - Requires 1.2.2.
  - Wire skip links, heading focus after client-side route changes, and a route
    announcement live region.
  - See
    `docs/building-accessible-and-responsive-progressive-web-applications.md`
    §5.3; `docs/wildside-pwa-design.md` §Accessible client-side routing; and
    `docs/pure-accessible-and-localizable-react-components.md` §§3.1 and Testing
    Strategies.
  - Success: Playwright keyboard tests prove focus moves only after client-side
    navigation, not on initial load.
- [ ] 1.2.4. Model the user experience graph as route-state fixtures and
      transition test cases.
  - Requires 1.2.2.
  - Generate or hand-maintain a typed state catalogue covering the 74 states,
    205 transitions, API contracts, and testing recommendations in the graph.
  - See `docs/wildside-ux-state-graph-v0.1.json` `states`, `transitions`,
    `apiContracts`, and `testingRecommendations`.
  - Success: every state in the graph has a route, parallel runtime state, or
    documented deferral mapping.

### 1.3. Build the validated data and synchronization boundary

This step answers whether the front-end can trust backend and fixture data
through one schema boundary. The outcome informs query hooks, optimistic
mutations, offline persistence, and future generated clients. See
`docs/wildside-pwa-data-model.md` §§Shared primitives, User state, Outbox, and
Suggested inbound endpoints; `spec/openapi.json`; and `spec/asyncapi.yaml`.

- [ ] 1.3.1. Generate and wrap the OpenAPI REST client with schema-validated
      fetchers.
  - Requires 1.1.2.
  - Regenerate client code with Orval, preserve abort-signal support, and wrap
    responses with Zod schemas at the fetch boundary.
  - See `spec/openapi.json`; `docs/wildside-pwa-design.md` §§Data access and API
    integration and Schema validation boundary; and
    `docs/wildside-pwa-data-model.md` §Suggested inbound endpoints.
  - Success: login, current user, interests, preferences, annotations, notes,
    progress, and health endpoints have typed query or mutation functions.
- [ ] 1.3.2. Add query-key factories and local-first QueryClient defaults for
      domain data.
  - Requires 1.3.1.
  - Configure stale time, garbage-collection time, retry policy, network mode,
    cache hydration, and query-key hierarchy for catalogue, preferences, routes,
    annotations, offline bundles, and walk sessions.
  - See `docs/local-first-react.md` §§Server State Synchronization,
    Configuration Deep Dive, and The Core Integration Strategy; and
    `docs/wildside-pwa-design.md` §§Core boundary and Offline-first and
    local-first persistence.
  - Success: cached domain data is not duplicated in a client-state store and is
    not garbage-collected before offline reuse.
- [ ] 1.3.3. Introduce Dexie storage for outbox and offline bundle manifests.
  - Requires 1.3.2.
  - Implement the minimal schema for outbox items and offline bundles while
    leaving tile bytes to Cache Storage for the Minimum Viable Product (MVP).
  - See `docs/wildside-pwa-data-model.md` §§Outbox, Idempotency contract, and
    Frontend persistence; and `docs/local-first-react.md` §§Durable Offline
    Writes and Handling Large Offline Assets.
  - Success: offline mutations persist across reloads with stable
    idempotency-key identifiers.
- [ ] 1.3.4. Define the WebSocket event boundary for asynchronous updates.
  - Requires 1.3.1.
  - Add a typed connection module for `/ws` that correlates display-name events
    now and leaves a documented extension point for route-generation progress.
  - See `spec/asyncapi.yaml`; `docs/local-first-react.md` §Real-Time Data Flow;
    and `docs/wildside-ux-state-graph-v0.1.json` `apiContracts.routeGeneration`.
  - Success: WebSocket messages patch or invalidate TanStack Query caches and
    never mutate view state directly.

### 1.4. Make accessibility, semantics, and documentation gates executable

This step answers whether the delivery loop can reject inaccessible, overly
utility-heavy, or undocumented UI before those patterns spread. The outcome
informs every later feature review. See
`docs/high-velocity-accessibility-first-component-testing.md`;
`docs/enforcing-semantic-tailwind-best-practice.md`; and
`docs/documentation-style-guide.md`.

- [ ] 1.4.1. Add the dual front-end test harness for fast component tests and
      accessibility scans.
  - Requires 1.1.2.
  - Wire Happy DOM or the existing fast harness for normal tests, plus a
    JSDOM-based `*.a11y.test.tsx` path for axe-compatible scans.
  - See `docs/high-velocity-accessibility-first-component-testing.md` §§I, II,
    and V; and `docs/wildside-pwa-design.md` §Testing strategy.
  - Success: `make test` runs both normal and accessibility-focused front-end
    tests or documents their separate CI entry points.
- [ ] 1.4.2. Add Playwright route, keyboard, axe, and accessibility-tree smoke
      tests.
  - Requires 1.2.2 and 1.4.1.
  - Cover the initial route shell, navigation groups, skip link, focus
    restoration, route announcements, and at least one dark and light theme
    viewport.
  - See `docs/high-velocity-accessibility-first-component-testing.md` §III and
    `docs/building-accessible-and-responsive-progressive-web-applications.md`
    §§4 and 6.
  - Success: the browser suite catches colour contrast, focus-order, and
    landmark regressions that JSDOM cannot validate.
- [ ] 1.4.3. Port semantic Tailwind, DaisyUI, and testing selector lint rules.
  - Requires 1.1.3.
  - Add Biome/Grit, class-length, near-duplicate, Semgrep, and Stylelint checks
    with project-specific allowlists.
  - See `docs/enforcing-semantic-tailwind-best-practice.md` §§3-11 and
    `docs/semantic-tailwind-with-daisyui-best-practice.md` §§1-9.
  - Success: clickable `div`/`span` patterns, raw colour utilities,
    `data-testid` selectors, and long repeated class lists fail or warn
    according to policy.
- [ ] 1.4.4. Document the front-end architecture and quality gates.
  - Requires steps 1.1-1.4.
  - Update repository documentation to describe the provider layout, route
    metadata, local-first persistence, and verification commands.
  - See `docs/documentation-style-guide.md`; `docs/wildside-pwa-design.md`; and
    `docs/v2a-front-end-stack.md`.
  - Success: a new contributor can run the front-end, regenerate the API client,
    run accessibility checks, and understand where each feature owns code.

## 2. Vertical slice 1: Catalogue-led onboarding and discovery

Idea: if Welcome, Discover, Explore, and Customise can render from localized
catalogue entities with cached fallback data, Wildside proves the UI can move
from mockup fixtures to backend-compatible projections before route generation
is fully live.

This slice delivers the first usable journey: launch the app, choose interests,
browse the route catalogue, refine route preferences, and preserve those choices
locally. It exercises entity-localized cards, descriptor registries,
OpenAPI-backed user preferences, and offline stale catalogue behaviour.

### 2.1. Prove catalogue entities can replace hard-coded card copy

This step answers whether the card architecture can drive the visible onboarding
and discovery surfaces without string duplication. The outcome informs every
later card, itinerary stop, and completion summary. See
`docs/data-model-driven-card-architecture.md` §§Purpose, Principles to enforce,
and Entity schemas; and `docs/wildside-pwa-data-model.md` §§Descriptors and
Catalogue.

- [ ] 2.1.1. Add shared entity, localization, media, and descriptor types.
  - Requires 1.3.1.
  - Define `EntityLocalizations`, `ImageAsset`, difficulty, tag, badge,
    interest, route summary, category, theme, collection, trending, and
    community-pick types.
  - See `docs/wildside-pwa-data-model.md` §§Shared primitives, Descriptors, and
    Catalogue; and `docs/data-model-driven-card-architecture.md` §§Shared model
    building blocks and Entity schemas by card type.
  - Success: TypeScript fixtures and API schema adapters share the same
    presentation-neutral entity vocabulary.
- [ ] 2.1.2. Implement deterministic locale resolution for entities and UI
      chrome.
  - Requires 2.1.1.
  - Add `pickLocalization`, supported-locale metadata, document `lang`/`dir`
    updates, and Fluent loading for page chrome and Accessible Rich Internet
    Applications (ARIA) scaffolding.
  - See `docs/v2a-front-end-stack.md` §§Localization stack and Data model-driven
    card architecture; `docs/wildside-pwa-design.md` §§Internationalization and
    Locale normalisation and RTL; and
    `docs/pure-accessible-and-localizable-react-components.md` §4.1.
  - Success: missing exact locales fall back predictably to `en-GB`, and RTL
    locales update document direction.
- [ ] 2.1.3. Reshape mockup catalogue fixtures into backend-compatible
      projections.
  - Requires 2.1.1 and 2.1.2.
  - Port route cards, categories, themes, collections, trending highlights,
    community picks, badges, tags, and interests from the v2a mockup shape.
  - See `docs/data-model-driven-card-architecture.md` §§Card inventory and
    Appendix A; `../wildside-mockup-v2a/docs/wildside-mockup-data-model.md`
    §§Catalogue and Descriptors; and `../wildside-mockup-v2a/src/app/data/`.
  - Success: the Explore and Discover fixture data no longer requires entity
    names or descriptions in Fluent bundles.
- [ ] 2.1.4. Add catalogue query adapters with fixture fallback and stale copy
      states.
  - Requires 1.3.2 and 2.1.3.
  - Query `GET /api/v1/catalogue/explore` when available and fall back to the
    shaped fixtures until backend catalogue endpoints are present.
  - See `docs/wildside-ux-state-graph-v0.1.json`
    `apiContracts.catalogueExplore`; `docs/wildside-pwa-data-model.md`
    §Catalogue snapshot API; and `docs/wildside-pwa-design.md` §§Service worker,
    manifest, and caching strategy.
  - Success: Explore can render fresh, stale-but-available, and unavailable
    catalogue states without changing components.

### 2.2. Deliver onboarding and discovery as one accessible journey

This step answers whether the first user journey is operable, localizable, and
responsive before backend route generation is needed. The outcome informs the
component contracts reused by wizard, map, and safety screens. See
`docs/sitemap.md` §§New User Onboarding and Quick Route Generation; and
`docs/wildside-ux-state-graph-v0.1.json` coverage for Welcome, Discover,
Explore, and Customize.

- [ ] 2.2.1. Implement the Welcome route and launch redirection.
  - Requires 1.2.2 and 1.2.3.
  - Port the mockup value proposition into a semantic landing screen with route
    metadata and responsive layout.
  - See `docs/sitemap.md` §§Route Structure and New User Onboarding;
    `docs/wildside-high-level-design.md` §Core Product Experience & Feature Set;
    and `docs/wildside-ux-state-graph-v0.1.json` `welcome.screen`.
  - Success: first app launch reaches `/welcome`, announces the page, and
    exposes one primary "get started" action.
- [ ] 2.2.2. Implement Discover interest selection with descriptor-backed toggle
      groups.
  - Requires 2.1.1 and 2.2.1.
  - Use Radix toggle behaviour, entity-localized labels, selected-count copy,
    and local preference draft state.
  - See `docs/wildside-pwa-data-model.md` §§Badge, tag, and interest theme and
    User profile and preferences;
    `docs/pure-accessible-and-localizable-react-components.md` §§3.1 and 4.2;
    and `docs/wildside-ux-state-graph-v0.1.json` `discover.interest_selection`.
  - Success: interest selection is keyboard-operable, localized, and survives
    route changes until persisted or queued.
- [ ] 2.2.3. Implement Explore catalogue browsing, search, and category filter
      states.
  - Requires 2.1.4.
  - Render categories, featured routes, collections, trending routes, and
    community picks from entity projections.
  - See `docs/wildside-pwa-data-model.md` §§Route summary, Route category,
    theme, and collection, and Trending and community picks;
    `docs/wildside-ux-state-graph-v0.1.json` states `explore.catalogue`,
    `explore.searching`, `explore.category_filtered`, `explore.stale_catalogue`,
    and `explore.catalogue_unavailable`; and
    `../wildside-mockup-v2a/docs/wildside-mockup-design.md` §Stage 1
    implementation notes.
  - Success: route cards use International System of Units (SI) unit formatting
    and no hard-coded entity copy.
- [ ] 2.2.4. Implement Customize preference controls and planned generation
      entry points.
  - Requires 2.2.3.
  - Port sliders, segment toggles, surface options, route preview selections, an
    explicit "Popular Hotspots" / "Hidden Gems" control mapped to
    `discoveryMix`, and disabled/planned generate states.
  - See `docs/wildside-ux-state-graph-v0.1.json` states `customize.editing`,
    `customize.preview_selected`, and `customize.generate_planned`;
    `docs/wildside-high-level-design.md` §Route Generation Controls; and
    `../wildside-mockup-v2a/docs/wildside-mockup-design.md` `/customize`
    localization strategy.
  - Success: Customize can generate a valid route draft object, including
    `discoveryMix`, without making a backend request.
- [ ] 2.2.5. Add bottom navigation and route-group affordances for the catalogue
      journey.
  - Requires 2.2.2, 2.2.3, and 2.2.4.
  - Mirror the sitemap navigation groups while keeping labels localized and
    `aria-current` accurate.
  - See `docs/sitemap.md` §Navigation Groups;
    `docs/enforcing-semantic-tailwind-best-practice.md` §§Landmarks and slot
    semantics and Stateful slots; and
    `docs/semantic-tailwind-with-daisyui-best-practice.md` §§2 and 5.
  - Success: bottom navigation is reachable by keyboard and route state is
    reflected through semantic attributes.

### 2.3. Persist preferences and demonstrate repeatable seed data

This step answers whether interest and preference writes can follow the
local-first contract while still being demonstrable with deterministic backend
sample data. The outcome informs later safety and wizard writes. See
`docs/backend-sample-data-design.md`; `spec/openapi.json`; and
`docs/wildside-pwa-data-model.md` §User state.

- [ ] 2.3.1. Implement preferences and interests query/mutation hooks.
  - Requires 1.3.1 and 2.2.2.
  - Wrap `GET/PUT /api/v1/users/me/preferences` and
    `PUT /api/v1/users/me/interests` with optimistic updates, revision checks,
    idempotency keys where applicable, and offline queueing.
  - See `spec/openapi.json`; `docs/wildside-pwa-data-model.md` §§User profile
    and preferences and Idempotency contract; and
    `docs/wildside-ux-state-graph-v0.1.json` `apiContracts.userPreferences`.
  - Success: stale preference writes surface `409 Conflict` as a user-resolvable
    sync conflict instead of silently overwriting data.
- [ ] 2.3.2. Add guest and authenticated preference resolution.
  - Requires 2.3.1.
  - Resolve `auth.unknown`, `auth.guest`, and `auth.authenticated` runtime
    states without blocking unauthenticated catalogue browsing.
  - See `docs/wildside-ux-state-graph-v0.1.json` assumption `A003` and auth
    states; `spec/openapi.json` paths `/api/v1/login` and `/api/v1/users/me`;
    and `docs/wildside-pwa-design.md` §Data access and API integration.
  - Success: Discover and Customize work as guest flows and merge or persist
    preferences after session resolution.
- [ ] 2.3.3. Add demo-data documentation and UI assumptions for seeded users.
  - Requires 2.3.2.
  - Document how backend example data populates users and preferences for demos,
    and how the front-end should behave when descriptor UUIDs are not yet backed
    by catalogue tables.
  - See `docs/backend-sample-data-design.md` §§Purpose, Data model alignment,
    and Future considerations.
  - Success: demo seed limitations are visible to implementers and do not leak
    as unexplained UI failures.

### 2.4. Verify the catalogue slice as a user-facing flow

This step answers whether the first vertical slice can be trusted as an
accessible Progressive Web App flow rather than just a set of rendered screens.
The outcome sets the verification template for later slices. See
`docs/high-velocity-accessibility-first-component-testing.md` and
`docs/wildside-ux-state-graph-v0.1.json` `testingRecommendations.T001`.

- [ ] 2.4.1. Add component and hook tests for entity localization, descriptor
      resolution, and unit formatting.
  - Requires 2.1.2 and 2.1.3.
  - Cover fallback locale ordering, International System of Units distance and
    duration formatting, and descriptor registry lookups.
  - See `docs/data-model-driven-card-architecture.md` §§Localization handling
    rules and Attribute identifier strategy; and `docs/v2a-front-end-stack.md`
    §§Locale resolution and Descriptor registries.
  - Success: entity cards do not render missing keys or raw International System
    of Units values.
- [ ] 2.4.2. Add accessibility tests for Welcome, Discover, Explore, and
      Customize.
  - Requires 2.2.1 through 2.2.4.
  - Scan rendered states with axe and assert role/name queries for primary
    actions, filters, sliders, and navigation.
  - See `docs/high-velocity-accessibility-first-component-testing.md` §§2.2 and
    2.3; and
    `docs/building-accessible-and-responsive-progressive-web-applications.md`
    §§4.1-4.4.
  - Success: tests use accessible queries only, with no `data-testid` fallback
    for ordinary controls.
- [ ] 2.4.3. Add Playwright coverage for onboarding, stale catalogue, and
      offline fallback states.
  - Requires 2.1.4 and 2.2.5.
  - Exercise Welcome -> Discover -> Explore, route announcement, keyboard
    navigation, stale catalogue copy, and no-network fallback.
  - See `docs/wildside-ux-state-graph-v0.1.json` `testingRecommendations.T001`
    and
    `docs/building-accessible-and-responsive-progressive-web-applications.md`
    §§5.1 and 6.
  - Success: the flow remains usable at mobile and wider responsive breakpoints
    with service-worker caching disabled and enabled.

## 3. Vertical slice 2: Generated walks and stable map experience

Idea: if the route wizard, asynchronous generation states, and MapLibre views
can share one route-plan model and one stable map provider, Wildside proves the
core product promise before offline downloads and completion summaries depend on
it.

This slice delivers the main Wildside loop: draft a walk, request generation,
track progress, review the result, inspect stops on a stable map, save the
route, and edit notes or progress. It exercises the hardest integration risks:
async route state, map canvas ownership, WebSocket cache updates, and idempotent
user-authored mutations.

### 3.1. Turn route drafts into asynchronous generation requests

This step answers whether the UI can model route generation as a durable
workflow instead of a synchronous button click. The outcome informs Map,
Itinerary, Saved, and Walk completion. See
`docs/wildside-ux-state-graph-v0.1.json` assumption `A004`;
`docs/wildside-pwa-data-model.md` §Generated routes; and
`docs/wildside-high-level-design.md` §The Secret Sauce.

- [ ] 3.1.1. Add route draft, request, status, and route-plan schemas.
  - Requires 1.3.1 and 2.2.4.
  - Model route preferences, generation request IDs, statuses, route geometry,
    ordered stops, inline Points of Interest (POIs), Point of
    Interest (POI) narrative snippet metadata, cache state,
    attribution, and sparse-data errors.
  - See `docs/wildside-pwa-data-model.md` §Generated routes and
    `docs/wildside-ux-state-graph-v0.1.json` states `route_generation.draft`,
    `route_generation.data_sparse`, and `route_generation.failed`; and
    `docs/wildside-high-level-design.md` §§In-Walk Navigation Experience and
    AI/LLM Integration Strategy.
  - Success: route plans and narrative snippets can be rendered offline from one
    persisted object, with missing snippets shown as recoverable loading or
    unavailable states.
- [ ] 3.1.2. Implement route-generation mutation and polling hooks.
  - Requires 3.1.1.
  - Wrap planned `POST /api/v1/routes`, `GET /api/v1/routes/{requestId}`, and
    `GET /api/v1/routes/{routeId}` contracts with idempotency keys and
    retry-safe cache updates.
  - See `docs/wildside-ux-state-graph-v0.1.json` `apiContracts.routeGeneration`;
    `docs/wildside-pwa-data-model.md` §Suggested inbound endpoints; and
    `docs/local-first-react.md` §§Part A and Handling Offline Mutations.
  - Success: duplicate retries do not create duplicate generated walks.
- [ ] 3.1.3. Integrate WebSocket progress as Query cache patches or
      invalidations.
  - Requires 1.3.4 and 3.1.2.
  - Map queued, progress, succeeded, failed, conflict, and cancelled events into
    the same route-generation cache state used by polling.
  - See `spec/asyncapi.yaml`; `docs/local-first-react.md` §Part B; and
    `docs/wildside-ux-state-graph-v0.1.json` states `route_generation.queued`,
    `route_generation.progress`, `route_generation.succeeded`,
    `route_generation.conflict`, and `route_generation.cancelled`.
  - Success: progress UI behaves the same whether status arrives by polling or
    by WebSocket event.

### 3.2. Deliver the wizard as the guided route-generation surface

This step answers whether the multistep wizard can orchestrate preferences,
safety choices, and generated-route review without invalid UI states. The
outcome informs the state-machine policy for later complex workflows. See
`docs/sitemap.md` §Nested Routes; `docs/wildside-ux-state-graph-v0.1.json`
Wizard coverage; and `docs/local-first-react.md` §Orchestrating Complex
Client-Side Logic with XState.

- [ ] 3.2.1. Implement wizard step 1 for duration and interests.
  - Requires 2.2.2 and 3.1.1.
  - Reuse interest descriptors, unit labels, and route-draft state.
  - See `docs/sitemap.md` §Route Structure;
    `docs/wildside-ux-state-graph-v0.1.json` `wizard.step1`; and
    `../wildside-mockup-v2a/docs/wildside-mockup-design.md` `/wizard`
    localization strategy.
  - Success: Step 1 can be completed by keyboard and updates only route-draft
    client state.
- [ ] 3.2.2. Implement wizard step 2 for discovery and accessibility
      preferences.
  - Requires 3.2.1.
  - Reuse safety descriptors where labels overlap, expose the "Popular Hotspots"
    / "Hidden Gems" `discoveryMix` control, and keep draft state separate from
    persisted preferences until submission.
  - See `docs/wildside-ux-state-graph-v0.1.json` `wizard.step2`;
    `docs/wildside-pwa-data-model.md` §§Safety toggles and presets; and
    `docs/pure-accessible-and-localizable-react-components.md` §2.2; and
    `docs/wildside-high-level-design.md` §Point of Interest (POI)
    Scoring & Personalization Algorithm.
  - Success: slider and toggle summaries remain localized and deterministic in
    tests, and generated route requests carry the selected `discoveryMix`.
- [ ] 3.2.3. Implement wizard step 3 review and generation transition.
  - Requires 3.1.2 and 3.2.2.
  - Show draft summary, route-generation states, generated stops, weather or
    highlight panels, and save confirmation affordances.
  - See `docs/wildside-ux-state-graph-v0.1.json` states `wizard.step3_review`
    and `wizard.saved_dialog`; and
    `../wildside-mockup-v2a/docs/wildside-mockup-design.md` `/wizard`
    localization strategy.
  - Success: generating, success, failed, and retry states are visibly and
    programmatically distinct.
- [ ] 3.2.4. Add a reducer or state-machine wrapper for wizard transitions.
  - Requires 3.2.1 through 3.2.3.
  - Encode next, back, reset, save-dialog, and generation transitions so
    impossible combinations cannot render.
  - See `docs/pure-accessible-and-localizable-react-components.md` §2.2 and
    `docs/local-first-react.md` §§Orchestrating Complex Client-Side Logic with
    XState and The Division of Labor.
  - Success: transition tests cover the user experience graph wizard edges
    without relying on incidental component state.

### 3.3. Keep MapLibre stable while overlays change

This step answers whether the map canvas can remain imperative and durable while
React overlays, tabs, and route state update around it. The outcome informs
offline tile caching and navigation. See `docs/wildside-pwa-design.md` §Map
architecture; and `docs/wildside-ux-state-graph-v0.1.json` assumption `A006`.

- [ ] 3.3.1. Implement a lazy MapLibre wrapper with graceful fallback.
  - Requires 3.1.1.
  - Load MapLibre and CSS lazily, register RTL text support, and render an
    accessible fallback when WebGL or style loading fails.
  - See `docs/v2a-front-end-stack.md` §Map stack; `docs/wildside-pwa-design.md`
    §§Map architecture and Locale normalisation and RTL; and
    `docs/wildside-ux-state-graph-v0.1.json` states `map.canvas_error` and
    `map.location_denied`.
  - Success: map load failures do not block route details, stops, or notes.
- [ ] 3.3.2. Add `MapStateProvider` for viewport, highlights, layers, and
      selected Points of Interest.
  - Requires 3.3.1.
  - Keep the map instance in a ref and expose imperative helpers for overlays.
  - See `docs/wildside-pwa-design.md` §Map state provider and
    `../wildside-mockup-v2a/docs/wildside-mockup-design.md` §Map state
    persistence plan.
  - Success: switching overlays and hovering stops does not recreate the
    MapLibre instance.
- [ ] 3.3.3. Implement route-start and location permission user experience.
  - Requires 3.3.2.
  - Support current Global Positioning System (GPS),
    dropped-pin start selection, map-centre fallback, last known location, and
    denied-permission recovery without blocking route browsing.
  - See `docs/wildside-high-level-design.md` §Route Generation Controls;
    `docs/wildside-ux-state-graph-v0.1.json` states
    `map.location_permission_prompt` and `map.location_denied`; and
    `docs/wildside-pwa-design.md` §§Map architecture and Accessible client-side
    routing.
  - Success: users can create a valid route draft after granting location,
    denying location, or manually placing a start pin.
- [ ] 3.3.4. Implement map-led quick generation as a primary route surface.
  - Requires 3.3.3 and 3.1.2.
  - Let users pan and zoom the map, inspect tappable Points of Interest, tune
    duration, interests, `discoveryMix`, and start point, then request a
    generated walk from the map without entering the full wizard.
  - See `docs/wildside-high-level-design.md` §§Interactive Map & Discovery and
    Route Generation Controls; `docs/wildside-ux-state-graph-v0.1.json` states
    `map.quick.map_tab`, `route_generation.requesting`, and
    `route_generation.data_sparse`; and `docs/sitemap.md` §Quick Route
    Generation.
  - Success: `/map/quick` can generate, retry, or broaden a walk request from
    map context with keyboard-operable controls and recoverable sparse-data
    copy.
- [ ] 3.3.5. Implement Quick Map tabs for map, stops, and notes.
  - Requires 3.3.4.
  - Use Radix tabs, hash-aware tab selection, route draft controls, Point of
    Interest highlights, and localized notes placeholders.
  - See `docs/wildside-ux-state-graph-v0.1.json` states `map.quick.map_tab`,
    `map.quick.stops_tab`, and `map.quick.notes_tab`; `docs/sitemap.md` §Nested
    Routes; and `../wildside-mockup-v2a/docs/wildside-mockup-design.md` §Stage 2
    implementation notes.
  - Success: deep links select the correct tab and `aria-selected` matches
    visual state.
- [ ] 3.3.6. Implement Itinerary tabs and navigation-active states.
  - Requires 3.3.5.
  - Render route plan geometry, ordered stops, Point of Interest detail dialogs,
    share dialog, navigation active and paused states, and progress events.
  - See `docs/wildside-ux-state-graph-v0.1.json` states `itinerary.map_tab`,
    `itinerary.stops_tab`, `itinerary.notes_tab`, `itinerary.navigation_active`,
    and `itinerary.navigation_paused`; and `docs/wildside-high-level-design.md`
    §In-Walk Navigation Experience.
  - Success: the user can start, pause, resume, and complete a walk without
    losing the map viewport or selected stop.
- [ ] 3.3.7. Add pedestrian instructions and degraded-location recovery.
  - Requires 3.3.6.
  - Render next-turn instructions, upcoming Point of Interest highlights,
    off-route copy, degraded GPS-confidence states, pause/resume affordances,
    and manual progress fallback when reliable tracking is unavailable.
  - See `docs/wildside-high-level-design.md` §In-Walk Navigation Experience;
    `docs/wildside-ux-state-graph-v0.1.json` states
    `itinerary.navigation_active`, `itinerary.navigation_paused`, and
    `map.location_denied`; and `docs/wildside-pwa-data-model.md` §§Notes and
    progress and Walk session and completion.
  - Success: active navigation remains usable when Global Positioning System
    confidence drops, the user leaves the route, or the device is offline.
- [ ] 3.3.8. Implement Saved route tabs, empty state, favourite, share, and
      start-route transitions.
  - Requires 3.3.6.
  - Reuse route-plan and annotation models while preserving the saved-route user
    experience graph states.
  - See `docs/wildside-ux-state-graph-v0.1.json` states `saved.empty`,
    `saved.map_tab`, `saved.stops_tab`, `saved.notes_tab`,
    `saved.favourite_toggled`, and `saved.share_dialog`; and `docs/sitemap.md`
    §Route Completion.
  - Success: empty and populated saved states are both accessible and
    localizable.

### 3.4. Persist notes, progress, and generated route projections

This step answers whether generated walks can become user-owned local-first
state. The outcome informs completion summaries and offline downloads. See
`spec/openapi.json`; `docs/wildside-pwa-data-model.md` §§Notes and progress and
Walk session and completion; and `docs/wildside-ux-state-graph-v0.1.json`
`apiContracts.routeAnnotations`.

- [ ] 3.4.1. Implement route annotations read and write hooks.
  - Requires 1.3.3 and 3.3.6.
  - Wrap `GET /api/v1/routes/{route_id}/annotations`,
    `POST /api/v1/routes/{route_id}/notes`, and
    `PUT /api/v1/routes/{route_id}/progress`.
  - See `spec/openapi.json` route annotation paths and
    `docs/wildside-pwa-data-model.md` §§Notes and progress and Outbox.
  - Success: notes and progress writes use idempotency keys and can queue
    offline.
- [ ] 3.4.2. Add optimistic note and progress updates with conflict recovery.
  - Requires 3.4.1.
  - Roll back failed writes, expose retry affordances, and surface revision
    conflicts as `runtime.sync_conflict`.
  - See `docs/local-first-react.md` §§Part A and Advanced Considerations; and
    `docs/wildside-ux-state-graph-v0.1.json` states `runtime.sync_conflict` and
    `runtime.sync_draining`.
  - Success: a failed progress update never marks a stop as permanently visited
    without server or queued confirmation.
- [ ] 3.4.3. Add route-plan persistence for generated and saved walks.
  - Requires 3.1.2 and 3.3.8.
  - Persist route plans and related Points of Interest in the Query cache so
    Map, Saved, and Completion can render without refetching.
  - See `docs/wildside-pwa-data-model.md` §Generated routes and
    `docs/local-first-react.md` §Implementing Offline Persistence.
  - Success: reloading the app can restore the last generated route and saved
    route details from local storage.
- [ ] 3.4.4. Implement Point of Interest narrative snippet lifecycle and cache
      behaviour.
  - Requires 3.1.1 and 3.4.3.
  - Render loading, available, stale, unavailable, and refreshed snippet states
    for Point of Interest detail surfaces, preserving attribution and avoiding
    duplicate LLM requests after reload.
  - See `docs/wildside-high-level-design.md` §§In-Walk Navigation Experience and
    AI/LLM Integration Strategy; `docs/wildside-pwa-data-model.md` §Generated
    routes; and `docs/local-first-react.md` §Implementing Offline Persistence.
  - Success: cached route plans can show existing narrative snippets offline,
    while missing snippets degrade to explicit unavailable copy.

### 3.5. Verify the generation and map slice

This step answers whether the core Wildside loop is robust enough to build
offline and completion features on top. See
`docs/wildside-ux-state-graph-v0.1.json` `testingRecommendations.T002`, `T003`,
and `T006`.

- [ ] 3.5.1. Add route-generation contract and state-transition tests.
  - Requires 3.1.1 through 3.1.3.
  - Cover idempotency-key submission, polling and WebSocket convergence,
    conflict, sparse data, cancellation, failure, and retry.
  - See `docs/wildside-ux-state-graph-v0.1.json` `testingRecommendations.T002`;
    `spec/openapi.json`; and `spec/asyncapi.yaml`.
  - Success: route-generation tests prove retry safety without a live backend.
- [ ] 3.5.2. Add map provider stability and overlay tests.
  - Requires 3.3.2 through 3.3.8.
  - Mock MapLibre and assert instance stability, layer toggles, Point of
    Interest highlights, tab switching, route-start fallback, and hash deep
    links.
  - See `docs/wildside-ux-state-graph-v0.1.json` `testingRecommendations.T003`
    and `../wildside-mockup-v2a/tests/map-state-provider.test.ts`.
  - Success: tests fail if overlay updates tear down the map canvas.
- [ ] 3.5.3. Add Playwright coverage for wizard-to-itinerary and saved-route
      flows.
  - Requires 3.2.3, 3.3.6, 3.3.7, 3.3.8, and 3.4.4.
  - Exercise keyboard flow through wizard steps, generated-route success,
    itinerary tab navigation, pedestrian instructions, degraded-Global
    Positioning System recovery, narrative snippet fallback, saved-route
    actions, and share dialogs.
  - See `docs/high-velocity-accessibility-first-component-testing.md` §III and
    `docs/wildside-ux-state-graph-v0.1.json` `testingRecommendations.T006`.
  - Success: a user can reach completion-ready navigation using only keyboard
    controls in the browser suite.

## 4. Vertical slice 3: Offline, safety, and completion trust

Idea: if offline bundles, safety preferences, walk sessions, and completion
summaries all reuse the same local-first outbox and route-plan model, Wildside
can offer the premium reliability promised by the Progressive Web App without
adding a second state system.

This slice completes the local-first promise. It makes the app installable,
defines service-worker cache policies, downloads route or region bundles,
persists safety preferences, records walk sessions, and renders a
non-fitness-style completion summary.

### 4.1. Make the app installable and explicit about cache behaviour

This step answers whether the app shell can behave like a Progressive Web App
before heavy offline bundle work begins. The outcome informs update user
experience, runtime cache policies, and offline affordances. See
`docs/wildside-pwa-design.md` §Service worker, manifest, and caching strategy;
and `docs/building-accessible-and-responsive-progressive-web-applications.md`
§§1-2.

- [ ] 4.1.1. Add the Web App Manifest and installability metadata.
  - Requires 1.1.3 and 1.2.2.
  - Define name, short name, icons, `start_url`, display mode, scope, theme
    colours, shortcuts, and maskable icon assets.
  - See
    `docs/building-accessible-and-responsive-progressive-web-applications.md`
    §1.1 and `docs/wildside-pwa-design.md` §Service worker, manifest, and
    caching strategy.
  - Success: Lighthouse recognises the app as installable in a production
    preview build.
- [ ] 4.1.2. Add a service worker with app-shell precache and navigation
      fallback.
  - Requires 4.1.1.
  - Cache built assets, serve the app shell for client-side routes, and avoid
    immediate `skipWaiting()` unless the update prompt is implemented. Validate
    service-worker registration and installability only in secure contexts:
    Hypertext Transfer Protocol Secure (HTTPS), with `http://localhost` allowed
    as the browser-enforced development exception.
  - See
    `docs/building-accessible-and-responsive-progressive-web-applications.md`
    §§1.2, 2.1, and 2.2; and `docs/wildside-pwa-design.md` §Service worker,
    manifest, and caching strategy.
  - Success: `/wizard/step-2` and `/map/quick` deep links load offline after a
    successful precache, and registration checks fail outside HTTPS or
    `http://localhost`.
- [ ] 4.1.3. Implement runtime cache policies for catalogue, route plans, status
      requests, and tiles.
  - Requires 4.1.2 and 3.4.3.
  - Use network-first cached fallback for catalogue, network-first or
    network-only route status, and cache-first tile requests with bundle-aware
    metadata.
  - See `docs/wildside-pwa-design.md` §Service worker, manifest, and caching
    strategy; `docs/local-first-react.md` §Where the Tile Bytes Live; and
    `docs/building-accessible-and-responsive-progressive-web-applications.md`
    §2.2.
  - Success: cache strategy decisions are encoded in service-worker tests and
    documented beside the implementation.
- [ ] 4.1.4. Add service-worker update and network-status runtime states.
  - Requires 4.1.2.
  - Render update-available, online, offline, queued, draining, partial drain,
    and conflict affordances without blocking the active route.
  - See `docs/wildside-ux-state-graph-v0.1.json` runtime states and
    `docs/building-accessible-and-responsive-progressive-web-applications.md`
    §5.1.
  - Success: status changes are announced politely and do not steal focus.

### 4.2. Deliver offline bundle lifecycle

This step answers whether route and region downloads can be managed without
putting tile bytes into React state. The outcome informs storage pressure
handling and paid reliability features. See `docs/wildside-pwa-data-model.md`
§Offline bundles; `docs/wildside-ux-state-graph-v0.1.json` Offline coverage; and
`docs/local-first-react.md` §Handling Large Offline Assets.

- [ ] 4.2.1. Implement offline bundle manifest queries and mutations.
  - Requires 1.3.3 and 4.1.3.
  - Wrap planned `GET/POST/DELETE /api/v1/offline/bundles` endpoints and the
    local manifest table with idempotent create/delete outbox items.
  - See `docs/wildside-pwa-data-model.md` §§Offline bundle manifest, Outbox, and
    Suggested inbound endpoints; and `docs/wildside-ux-state-graph-v0.1.json`
    `apiContracts.offlineBundles`.
  - Success: bundle manifests can be created and deleted while offline.
- [ ] 4.2.2. Implement Offline dashboard, add-area dialog, manage mode, delete
      undo, and storage pressure states.
  - Requires 4.2.1.
  - Port the v2a offline user experience with semantic dialogs, progress
    indicators, and localized size formatting.
  - See `docs/wildside-ux-state-graph-v0.1.json` states `offline.dashboard`,
    `offline.add_area_dialog`, `offline.manage_mode`,
    `offline.delete_pending_undo`, and `offline.storage_pressure`; and
    `docs/data-model-driven-card-architecture.md` §Offline entities.
  - Success: every destructive action has an undo or confirmation path and an
    accessible name.
- [ ] 4.2.3. Implement tile prefetch, progress, failure, retry, and eviction
      integration.
  - Requires 4.2.1 and 4.1.3.
  - Coordinate service-worker Cache Storage, bundle status transitions, quota
    errors, and retry.
  - See `docs/wildside-pwa-data-model.md` §Tile storage;
    `docs/local-first-react.md` §§Where the Tile Bytes Live and Computing Tile
    URLs from Bounds; and `docs/wildside-ux-state-graph-v0.1.json` states
    `offline.bundle_queued`, `offline.bundle_downloading`,
    `offline.bundle_complete`, and `offline.bundle_failed`.
  - Success: tile bytes are not stored in React state or TanStack Query.

### 4.3. Persist safety preferences with conflict-aware user experience

This step answers whether safety and accessibility preferences can share the
same preference aggregate and conflict policy created during onboarding. The
outcome informs future accessibility presets and route-generation filters. See
`docs/wildside-pwa-data-model.md` §§Safety toggles and presets and User state.

- [ ] 4.3.1. Implement safety descriptor registries and preset mappings.
  - Requires 2.1.1.
  - Model safety toggles, presets, applied toggle IDs, icons, and default states
    as localized descriptors.
  - See `docs/wildside-pwa-data-model.md` §Safety toggles and presets and
    `docs/data-model-driven-card-architecture.md` §Safety preferences.
  - Success: the backend stores semantic toggle IDs, not UI class names or
    English labels.
- [ ] 4.3.2. Implement Safety and Accessibility screen accordions, toggles,
      presets, and saved dialog.
  - Requires 4.3.1 and 2.3.1.
  - Replace mockup alert-only preset behaviour with state updates behind the
    same saved-dialog pattern.
  - See `docs/wildside-ux-state-graph-v0.1.json` assumption `A007` and states
    `safety.preferences`, `safety.toggle_changed`, `safety.preset_alert`, and
    `safety.saved_dialog`; and
    `../wildside-mockup-v2a/docs/wildside-mockup-design.md`
    `/safety-accessibility` localization strategy.
  - Success: applying a preset marks preferences dirty and can be saved or
    queued offline.
- [ ] 4.3.3. Add conflict UI for stale safety preference writes.
  - Requires 4.3.2 and 1.3.3.
  - Offer reload or merge affordances when the backend returns `409 Conflict`.
  - See `docs/wildside-ux-state-graph-v0.1.json` `testingRecommendations.T005`;
    `docs/wildside-pwa-data-model.md` §User profile and preferences; and
    `spec/openapi.json` `/api/v1/users/me/preferences`.
  - Success: stale writes do not discard local choices without user action.

### 4.4. Record walks and render completion summaries

This step answers whether a generated route can become a completed experience
without drifting into fitness-app metrics. The outcome closes the core Minimum
Viable Product loop from discovery to post-walk summary. See
`docs/wildside-high-level-design.md` §Post-Walk Summary and
`docs/wildside-pwa-data-model.md` §Walk session and completion.

- [ ] 4.4.1. Implement walk-session creation and completion mutation hooks.
  - Requires 3.4.1 and 3.4.3.
  - Use client-generated UUIDs, route IDs, start and end times, progress, and
    highlighted Point of Interest IDs; queue writes offline when needed.
  - See `docs/wildside-pwa-data-model.md` §Walk session and completion;
    `docs/wildside-ux-state-graph-v0.1.json` `apiContracts.walkSessions`; and
    `docs/local-first-react.md` §Durable Offline Writes.
  - Success: completing a walk can be retried without creating duplicate
    sessions.
- [ ] 4.4.2. Implement Walk Complete summary, rating toast, share dialog, save,
      and remix transitions.
  - Requires 4.4.1 and 3.3.8.
  - Render distance, duration, favourite moments, sharing affordances, save
    transition, and remix link back to the wizard.
  - See `docs/wildside-ux-state-graph-v0.1.json` states `walk_complete.summary`,
    `walk_complete.rating_toast`, and `walk_complete.share_dialog`;
    `docs/sitemap.md` §Route Completion; and
    `docs/data-model-driven-card-architecture.md` §Walk completion.
  - Success: completion copy emphasizes experience and discovered places, not
    performance scoring.
- [ ] 4.4.3. Connect active navigation completion to Saved route state.
  - Requires 4.4.1 and 4.4.2.
  - Persist completion state, update route progress, and route users to Saved or
    Wizard according to the user experience graph.
  - See `docs/wildside-ux-state-graph-v0.1.json` `testingRecommendations.T006`
    and transitions from `itinerary.navigation_active` to
    `walk_complete.summary`.
  - Success: saved routes reflect completion progress after reload.

### 4.5. Verify offline, safety, and completion reliability

This step answers whether local-first behaviour survives the real browser
conditions it is meant to handle. See `docs/wildside-ux-state-graph-v0.1.json`
`testingRecommendations.T004`, `T005`, and `T006`.

- [ ] 4.5.1. Add service-worker, manifest, and cache-policy tests.
  - Requires 4.1.1 through 4.1.4.
  - Cover manifest validity, navigation fallback, offline app-shell load, cache
    strategy routing, and update-available affordance.
  - See
    `docs/building-accessible-and-responsive-progressive-web-applications.md` §6
    and `docs/wildside-pwa-design.md` §Service worker, manifest, and caching
    strategy.
  - Success: Lighthouse Progressive Web App checks and local Playwright offline
    smoke tests pass in a preview build.
- [ ] 4.5.2. Add offline bundle lifecycle tests.
  - Requires 4.2.1 through 4.2.3.
  - Cover queued, downloading, complete, failed, retry, delete undo, quota
    failure, and tile-cache separation.
  - See `docs/wildside-ux-state-graph-v0.1.json` `testingRecommendations.T004`
    and `docs/local-first-react.md` §Where the Tile Bytes Live.
  - Success: tests prove manifest state is durable and tile bytes are outside
    React state.
- [ ] 4.5.3. Add safety conflict and walk-completion end-to-end tests.
  - Requires 4.3.3 and 4.4.3.
  - Exercise offline preference queueing, `409` conflict recovery, active
    navigation progress, completion, save, and remix.
  - See `docs/wildside-ux-state-graph-v0.1.json` `testingRecommendations.T005`
    and `T006`.
  - Success: queued writes drain with idempotency keys and completion remains
    accessible in at least one non-default locale.

## 5. Deferred extensions after the core Progressive Web App promise

Idea: if the core Progressive Web App is already trustworthy, accessible,
local-first, and boring to operate, the project can evaluate broader extensions
on product value instead of letting them destabilize the main release.

This phase collects work that the design documents mention but do not require
for the first production Progressive Web App slice. These tasks should not block
phases 1-4 unless a product decision explicitly promotes them into Minimum
Viable Product scope.

### 5.1. Evaluate account, auth, and profile expansion

This step answers which account features should graduate beyond background
session handling. See `docs/wildside-ux-state-graph-v0.1.json` assumption
`A003`, `spec/openapi.json`, and `spec/asyncapi.yaml`.

- [ ] 5.1.1. Decide whether a visible sign-in route belongs in the first
      production release.
  - Requires phase 2.
  - Compare the mockup `sign-in.html`, current OpenAPI login/current-user
    contracts, and the user experience graph's future auth region.
  - See `docs/wildside-ux-state-graph-v0.1.json` auth states;
    `spec/openapi.json` `/api/v1/login`; and
    `../wildside-mockup-v2a/public/mockups/sign-in.html`.
  - Success: auth either becomes a planned route slice with acceptance criteria
    or remains a background session concern.
- [ ] 5.1.2. Evaluate WebSocket display-name validation against REST session
      flows.
  - Requires 5.1.1.
  - Decide whether `DisplayNameRequest`, `InvalidDisplayName`, and `UserCreated`
    remain active client contracts.
  - See `spec/asyncapi.yaml`.
  - Success: unused WebSocket user events are either wired into auth user
    experience or documented as backend-only legacy scope.
- [ ] 5.1.3. Decide entitlement and free-tier user experience before premium
      controls ship.
  - Requires phase 4.
  - Define how free walk-generation limits, unlimited generation, expanded
    interest themes, and offline-map entitlement are presented without blocking
    the core generative loop.
  - See `docs/wildside-high-level-design.md` §§Risk 5: Market Adoption and
    Monetization and Post-Minimum Viable Product & Future Vision; and
    `docs/wildside-ux-state-graph-v0.1.json` assumption `A003`.
  - Success: entitlement is either promoted into a route/account slice with
    acceptance criteria or documented as out of scope for the first production
    release.

### 5.2. Evaluate richer catalogue and list pagination

This step answers whether list pagination is needed once catalogue screens move
beyond the initial snapshot. See `docs/keyset-pagination-design.md` and
`docs/wildside-pwa-data-model.md` §Catalogue snapshot API.

- [ ] 5.2.1. Decide which front-end list surfaces need keyset pagination.
  - Requires phase 2.
  - Audit users, catalogue collections, saved routes, Points of Interest, and
    notes for large-list behaviour.
  - See `docs/keyset-pagination-design.md` §§Overview and Goals, Paginated
    Response Envelope, and Cursor Semantics; and `spec/openapi.json`
    `/api/v1/users`.
  - Success: paginated screens either consume opaque cursors and links or stay
    explicitly snapshot-based.
- [ ] 5.2.2. Add pagination UI patterns only for promoted list surfaces.
  - Requires 5.2.1.
  - Use response `links.self`, `links.next`, and `links.prev` rather than
    constructing cursor URLs in the UI.
  - See `docs/keyset-pagination-design.md` §§Envelope Response Format and
    Hypermedia Navigation Links.
  - Success: users can page forward and backward without the front-end parsing
    cursor internals.

### 5.3. Evaluate native wrappers and advanced platform features

This step answers whether the Progressive Web App foundation is ready for
platform-specific distribution. See `docs/wildside-high-level-design.md`
§Post-Minimum Viable Product Frontend Roadmap: Desktop and Mobile.

- [ ] 5.3.1. Reassess Capacitor and Tauri packaging after Progressive Web App
      hardening.
  - Requires phase 4.
  - Validate service-worker, storage, geolocation, map, and offline tile
    behaviours inside native WebView constraints.
  - See `docs/wildside-high-level-design.md` §§Post-Minimum Viable Product &
    Future Vision and Post-Minimum Viable Product Frontend Roadmap: Desktop and
    Mobile.
  - Success: native wrapper work has a separate roadmap with platform-specific
    test gates.
- [ ] 5.3.2. Evaluate push notifications and background sync as progressive
      enhancements.
  - Requires phase 4.
  - Treat notifications and background sync as optional capability upgrades, not
    prerequisites for outbox correctness.
  - See
    `docs/building-accessible-and-responsive-progressive-web-applications.md`
    §5.4 and `docs/wildside-pwa-design.md` §Service worker, manifest, and
    caching strategy.
  - Success: unsupported browsers still drain the outbox on app resume and retry
    cadence.

### 5.4. Evaluate community, audio, and local-intent features

This step answers which higher-level product differentiators are worth adding
after the core generative loop is dependable. See
`docs/wildside-high-level-design.md` §§Post-Minimum Viable Product & Future
Vision and AI/LLM Integration Strategy.

- [ ] 5.4.1. Decide whether community ratings, reviews, and route sharing
      graduate from mockup affordances.
  - Requires phase 4.
  - Define backend contracts, moderation needs, privacy expectations, and card
    projections before adding visible write paths.
  - See `docs/wildside-high-level-design.md` §Advanced Features & Community
    Integration and `docs/wildside-pwa-data-model.md` §§Trending and community
    picks.
  - Success: community features have explicit data contracts rather than
    overloading completion or saved-route state.
- [ ] 5.4.2. Evaluate audio guides and on-device intent recognition as separate
      experiments.
  - Requires phase 4.
  - Keep generated audio, device model constraints, and natural-language route
    controls outwith the core Progressive Web App state machine until contracts
    are proven.
  - See `docs/wildside-high-level-design.md` §§Audio Guides and AI/LLM
    Integration Strategy.
  - Success: advanced media and intent features have prototype success criteria
    before entering the main roadmap.
- [ ] 5.4.3. Define feedback and reporting user experience for route and data
      quality.
  - Requires phase 4.
  - Decide whether users can report bad Points of Interest, unsafe route
    segments, stale narrative snippets, poor generated walks, or accessibility
    mismatches, and define moderation and privacy expectations before adding
    visible write paths.
  - See `docs/wildside-high-level-design.md` §§Risk 1: Data Quality and
    Maintenance and Advanced Features & Community Integration; and
    `docs/wildside-pwa-data-model.md` §§Notes and progress and Trending and
    community picks.
  - Success: feedback either has explicit data contracts and user-visible
    recovery copy or remains a documented post-Minimum Viable Product research
    item.
