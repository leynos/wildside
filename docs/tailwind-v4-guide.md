# Tailwind CSS v4 project guide

This guide captures the Tailwind CSS v4 rules that matter for Wildside. It is
not a copy of the upstream Tailwind documentation. Use the official Tailwind CSS
documentation for exhaustive utility and API reference material:
<https://tailwindcss.com/docs>.

## Installation and setup

Wildside should use the CSS-first Tailwind v4 model:

```css
@import "tailwindcss";

@source "./src/**/*.{ts,tsx,js,jsx,mdx}";

@theme {
  --font-sans: "Inter", system-ui, sans-serif;
  --spacing: 4px;
  --radius-card: 0.5rem;
}
```

Project-specific constraints:

- Theme values belong in CSS custom properties generated from
  `packages/tokens/`.
- Generated token outputs are build artefacts and should not become hand-edited
  design source.
- Explicit `@source` entries are preferred for non-standard source locations so
  production builds include every required utility.
- Raw palette utilities should be avoided in application markup where a DaisyUI
  semantic role exists.

## Framework integration

The current front-end stack uses Bun, Vite, React, Tailwind CSS v4, DaisyUI v5,
Radix UI and generated design tokens.

Implementation expectations:

- Keep Tailwind configuration close to the entry stylesheet unless a plugin needs
  JavaScript configuration.
- Use `@tailwindcss/postcss` or the Vite integration selected by the front-end
  stack decision.
- Keep build and check commands behind Makefile targets so local and CI usage do
  not drift.
- Validate Tailwind output through the semantic lint gates imported from the v2a
  mockup.

## Utility and variant rules

Tailwind utilities should be used for local layout, spacing and state-specific
presentation. Repeated product concepts should move into small semantic wrappers
or generated token roles.

Recommended patterns:

- Use data and ARIA variants for Radix state, for example
  `data-[state=open]:bg-primary`.
- Prefer `gap` over `space-x-*` and `space-y-*` for flex and grid layouts.
- Use container queries only when the component layout depends on its container,
  not as a substitute for page breakpoints.
- Use arbitrary values sparingly and prefer token-backed values such as
  `bg-(--color-primary)`.

## `@apply`, `@utility` and scoped CSS

Tailwind v4 only inlines plain utilities through `@apply`; variant prefixes and
plugin component classes should not be hidden inside `@apply`.

```css
@utility action-chip {
  @apply inline-flex items-center gap-2 rounded-field px-3 py-2 text-sm;
  background: var(--color-primary);
  color: var(--color-primary-content);
}

.action-chip[data-state="on"] {
  @apply ring-2;
}
```

Rules:

- Use `@utility` for project wrappers that need variant support.
- Use `@apply` only for Tailwind utilities, not DaisyUI component classes such
  as `btn` or `card`.
- Add `@reference` in component-scoped styles only when scoped CSS must resolve
  tokens or utilities from the main stylesheet.

## Migration notes

Tailwind v3 code should migrate through the dedicated
`tailwind-v3-v4-migration-guide.md`. New Wildside front-end work should target
Tailwind v4 conventions directly unless a documented compatibility bridge is in
place.
