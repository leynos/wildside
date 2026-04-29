# v2a front-end stack

This document describes the Wildside front-end stack in two layers:

- the stack currently declared and wired in `frontend-pwa/package.json`, and
- the fuller v2a application stack described elsewhere in this repository’s
  design and architecture documents.

That distinction matters because the repository currently contains a minimal
progressive web app (PWA) shell, while the broader v2a product architecture adds routing,
localization, map, local-first data, and orchestration tooling that is not yet
fully declared in `frontend-pwa/package.json`.

## Overview

The current Wildside progressive web app (PWA) is a client-side React application built with Bun,
Vite `^7.3.2`, React 19, React DOM 18, Tailwind CSS `^3`, DaisyUI `^4`,
TanStack Query, Zod, clsx, TypeScript, Vitest, and Orval.

The fuller v2a target stack described by the roadmap and mockup adds TanStack
Router, Tailwind CSS v4, DaisyUI v5, Radix UI primitives, i18next with Fluent
translation bundles, MapLibre GL JS, and the data-model-driven card
architecture for presenting domain entities.

The map canvas, tile rendering, and location-aware UI are specific to the
Wildside product domain. They are part of the Wildside mockup because it models
a map-based exploration application, not because every front-end in this repo
family would need them.

## Stack layers at a glance

### Current `frontend-pwa` stack

The current `frontend-pwa/package.json` declares:

- Bun,
- Vite `^7.3.2`,
- React 19,
- React DOM 18,
- TanStack Query 5,
- Tailwind CSS `^3`,
- DaisyUI `^4`,
- Zod 3,
- clsx 2,
- TypeScript 5,
- Vitest 3,
- Orval 8, and
- the current build, linting, token generation, and type checking toolchain described below.

### Full v2a application stack

The fuller v2a stack described across the repo’s architecture documents adds:

- **Zustand** for interactive client and UI state,
- **TanStack Query** for server-state fetching, caching, and synchronization,
- **Dexie** for durable browser-side storage of offline bundles, map tiles, and
  related heavier local data, and
- **XState** for modelling more complex interaction and workflow orchestration
  where a reducer or plain context store becomes too implicit.

In other words, the current PWA shell proves the build, token-generation, API-client, and
TanStack Query foundation, while the full product stack still needs the
presentation, navigation, localization, map, and richer local-first state
architecture.

## Runtime and build toolchain

- **Package manager and runner:** Bun drives local scripts, tests, and token
  generation via `package.json` and `bunfig.toml`.
- **Bundler and dev server:** Vite `^7.3.2` is the application bundler and
  development server.
- **React integration:** `@vitejs/plugin-react` handles JSX and React Fast
  Refresh.
- **Tailwind integration:** Tailwind CSS `^3` runs through
  `tailwind.config.js`, which consumes generated token presets and DaisyUI theme
  output.
- **Module format:** The project is ECMAScript modules (ESM)-only (`"type": "module"` in
  `package.json`).
- **Token integration:** `vite.config.ts` wires a design-token plugin so token
  outputs are generated before development, build, and preview workflows.

In practice, the front-end entry path is:

1. `src/main.tsx` boots React with `StrictMode` and `Suspense`.
2. `src/app/app.tsx` wires global providers.
3. `src/app/App.tsx` renders the current single-route shell.

## Core application framework

### React

The UI is built with React 19 and `react-dom/client`. The main bootstrap file,
`src/main.tsx`, mounts the single-page application (SPA) into `#root`,
provides a loading fallback via Suspense, and avoids rendering during tests.

### Routing

The current PWA does not declare TanStack Router and still renders a single
application (SPA) shell. The v2a target route tree is planned by the roadmap
and sitemap, but it is not yet implemented in `frontend-pwa`.

### State management

In the currently checked-in mockup, there is no declared global client-state
library such as Redux, Zustand, or XState in `package.json`. State is managed
through:

- React component state and hooks,
- the checked-in `ThemeProvider`, and
- route-local and component-local state for shell and screen behaviour.

That keeps the checked-in mockup closer to a React-plus-context architecture
than a state-machine or data-cache architecture.

For the fuller v2a application stack, the state split is broader:

- **Zustand** owns interactive client and UI state,
- **TanStack Query** owns server and synchronized domain state, and
- **XState** is the right fit for explicit multistep workflows or long-running
  interaction logic that benefits from a formal state machine.

## Styling and design system

### Tailwind CSS and DaisyUI

The current styling stack is built on Tailwind CSS `^3`, DaisyUI `^4`, and the
repository token package.

- `src/index.css` imports `@app/tokens/css/variables.css` and the Tailwind layer
  directives.
- `tailwind.config.js` loads generated Tailwind presets from
  `@app/tokens/dist/tw/preset.js`.
- DaisyUI theme output is imported from `@app/tokens/dist/daisy/theme.js`.

The styling approach is not utility-only. The project uses a hybrid of:

- Tailwind utilities,
- DaisyUI component classes such as `btn`, and
- semantic project classes defined in `src/index.css` and the files under
  `src/styles/`.

### Design tokens

Design tokens are a first-class part of the stack.

- `packages/tokens/build/style-dictionary.js` uses Style Dictionary to generate
  token artefacts.
- `pnpm --filter @app/tokens build` produces:
  - `packages/tokens/dist/css/variables.css` for runtime CSS variables,
  - `packages/tokens/dist/tw/preset.js` for Tailwind preset data, and
  - `packages/tokens/dist/daisy/theme.js` for DaisyUI theme data.
- `vite.config.ts` watches the generated token outputs and triggers a full-page
  reload when they change.

This means the front-end theme layer is not hand-maintained in one place.
Instead, the design source of truth lives in the token package, and both CSS and
Tailwind consume generated outputs.

### Theme handling

Theme selection is handled with a small React context rather than a theme
framework:

- `src/app/providers/theme-provider.tsx` persists the active theme in
  `localStorage`.
- The provider sets `data-theme` on the document root and body.
- The shipped theme names are `corbusier-mockup-night` and
  `corbusier-mockup-day`.

### PostCSS

The current PWA does not declare a project-level PostCSS configuration. Tailwind
is wired through `tailwind.config.js`, and any future Tailwind v4 migration
should document the new PostCSS or CSS-first integration in the same change that
adds the dependency upgrade.

## Component primitives and icons

The current UI component layer does not declare Radix UI packages. The checked-in
runtime depends on:

- `clsx` for conditional class composition.

Radix primitives remain part of the v2a target stack and should be introduced
through the roadmap task that reconciles the front-end platform design.

## Localization stack

The current PWA does not declare i18next, React i18next, or Fluent packages.
Localization is a v2a target capability and should be introduced with the
entity and UI-chrome localization tasks in the roadmap.

## Map stack

The current PWA does not declare MapLibre GL JS. Interactive map views are a
Wildside v2a target capability for itinerary, quick-walk, saved-route, tile, and
location flows.

- The fuller map stack is expected to lazy-load `maplibre-gl` and its CSS when
  that dependency is introduced.
- OpenMapTiles-backed styling supplies the target base map presentation.
- The target integration should register the MapLibre RTL text plugin when the
  runtime supports it.
- Shared viewport and layer state should live in a dedicated map-state module.

In the full v2a architecture, map-related persistence is also expected to touch
the broader local-first stack:

- TanStack Query for route and place data,
- Dexie for offline bundles, map tile storage, and other heavier offline
  artefacts, and
- UI state or workflow orchestration layered above that with Zustand and, where
  useful, XState.

## Testing and verification stack

The repository has a broader-than-average front-end verification setup.

### Unit and component tests

- `vitest` is the current front-end test runner.
- Root-level `make test-frontend` runs the front-end package tests through the
  workspace scripts.
- The richer v2a accessibility and Playwright harness is documented as roadmap
  import work, not current `frontend-pwa` package state.

### Accessibility-focused and end-to-end tests

The current `frontend-pwa/package.json` does not declare Testing Library,
Happy DOM, Playwright, or `axe-core` packages. Those tools are part of the v2a
target verification stack and should be imported through the roadmap tasks that
cover lint, accessibility, and semantic testing gates.

## Linting, type-checking, and semantic checks

The current repository quality stack uses Makefile entry points:

- `make fmt` runs Rust formatting and Biome formatting for front-end packages.
- `make lint-frontend` runs Biome checks for `frontend-pwa` and `packages`.
- `make typecheck` runs `tsc --noEmit` for TypeScript workspaces.
- `make markdownlint` validates Markdown using a local `markdownlint-cli2`
  binary when present, with a pinned Bun fallback.

The stricter v2a semantic checks, Stylelint rules, Semgrep rules, Fluent
variable linting, and Playwright accessibility gates are roadmap import work.

The current `frontend-pwa/tsconfig.json` includes:

- `strict`,
- `moduleResolution: "Bundler"`,
- `verbatimModuleSyntax`, and
- `noEmit`.

## Data model-driven card architecture

All v2a front ends — Wildside and Corbusier alike — share a common pattern for
presenting domain entities on cards, lists, and detail screens. Entity models
carry their own localized strings rather than delegating display-text
responsibility to the Fluent translation bundles. This keeps Fluent bundles
focused on UI chrome (button labels, Accessible Rich Internet Applications
(ARIA) labels, section headings, format
strings) while letting each entity own its names, descriptions, and badge text
per locale.

### Shared primitives

The following types form the shared vocabulary across all v2a front ends:

- **`LocaleCode`** — a branded string identifying a supported locale (e.g.
  `en-GB`, `ja`).
- **`LocalizedStringSet`** — a record mapping `LocaleCode` keys to translated
  display strings.
- **`EntityLocalizations`** — a per-entity bundle that groups together every
  `LocalizedStringSet` the entity needs (name, description, badge text, and so
  on).
- **`LocalizedAltText`** — a `LocalizedStringSet` specifically for image alt
  text, kept as a distinct type so that accessibility tooling can enforce its
  presence.
- **`ImageAsset`** — a reference to an image file together with its
  `LocalizedAltText`.

### Locale resolution

A small pure helper, `pickLocalization(localizations, locale)`, resolves a
`LocalizedStringSet` for the requested locale. If the exact locale is not
present, the helper falls back to `en-GB`. This single function is the only
place where locale-fallback logic lives, keeping the rest of the rendering code
free of null checks or conditional chains.

### Descriptor registries

Stable internal identifiers (e.g. `task-status:in-progress`, `priority:high`)
are resolved to localized display strings through descriptor registries. Each
registry maps an identifier to a descriptor object whose labels are
`LocalizedStringSet` values. This means that badge colours, icons, and display
names for status values, priority levels, and similar enumerations are defined
once and shared by every component that renders them.

### Folder layout

The conventional folder layout is:

- `src/app/domain/entities/` — TypeScript interfaces and type aliases for entity
  models and their localization shapes.
- `src/data/entities/` — fixture data used during the mockup phase, structured
  to match the entity interfaces exactly.
- `src/data/registries/` — descriptor registry modules that map stable
  identifiers to localized descriptors.

### Application-specific schemas

Each application maintains its own
`docs/data-model-driven-card-architecture.md`, which defines the
application-specific entity schemas, enumerations, and migration roadmap. That
document is the authoritative reference for which entities exist, what fields
they carry, and how the card architecture will evolve as the mockup matures
toward production data sources.

## Effective stack summary

For the shortest accurate summary of the checked-in mockup, the current
front-end stack is:

- Bun for scripts and package-manager-adjacent tooling,
- Vite `^7.3.2` for bundling and development,
- React 19 for the SPA runtime,
- React DOM 18 for rendering,
- TanStack Query 5 for server-state caching,
- Tailwind CSS `^3` plus DaisyUI `^4` for current styling,
- Style Dictionary for generated design tokens and themes,
- a data-model-driven card architecture for entity display strings,
- Biome, TypeScript, markdownlint, and Makefile validation for code quality,
  and
- Vitest for current front-end tests.

For map/location-based applications specifically, add a domain layer atop that
stack:

- MapLibre GL JS for interactive maps,
- OpenMapTiles-backed tile styling, and
- location-centric UI and state for itinerary and route experiences.

For the shortest accurate summary of the fuller v2a application architecture,
add:

- Zustand for interactive state,
- TanStack Query for server-state caching and synchronization,
- Dexie for offline bundle and map tile storage, and
- XState for explicit interaction orchestration where state machines are
  warranted.

## What is not currently declared in `frontend-pwa`

To avoid confusion, this section refers to `frontend-pwa/package.json`, not the
broader v2a target architecture described above.

The following target or common front-end tools are not currently declared in the
PWA runtime dependencies:

- TanStack Router,
- Radix UI,
- i18next, React i18next, and Fluent integration packages,
- MapLibre GL JS,
- TanStack Table,
- Redux,
- Next.js, Remix, or any server-side rendering (SSR) framework, and
- Framer Motion.
