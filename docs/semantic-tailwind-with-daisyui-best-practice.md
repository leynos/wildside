# Semantic Tailwind with daisyUI best practice

This guide defines the Wildside layering model for semantic HTML, Radix
behaviour, Tailwind CSS v4 utilities, daisyUI v5 roles and generated design
tokens. It is project guidance, not a replacement for upstream Tailwind or
daisyUI documentation.

## 0. Setup

Use a single entry stylesheet that imports Tailwind, registers daisyUI and
loads generated tokens.

```css
@import "tailwindcss";

@plugin "daisyui" {
  themes:
    wildside-day --default,
    wildside-night --prefersdark;
}

@source "./src/**/*.{ts,tsx,js,jsx,mdx}";
```

## 1. Mental model

The styling stack has five layers:

1. **Semantic HTML:** native elements and landmarks first.
2. **Headless behaviour:** Radix primitives provide state through ARIA and
   `data-*` attributes.
3. **daisyUI component classes:** `btn`, `card`, `input`, `menu` and related
   roles provide structure and theme-aware presentation.
4. **Tailwind utilities:** utilities refine local spacing, layout, responsive
   behaviour and state.
5. **Semantic wrappers:** small project classes encode repeated product intent.

## 2. Semantic HTML

Native elements should be preferred over ARIA-only replacements:

```html
<nav aria-label="Primary">
  <a class="link" href="/explore" aria-current="page">Explore</a>
</nav>

<main id="content">
  <section aria-labelledby="routes-heading">
    <h2 id="routes-heading">Routes</h2>
  </section>
</main>
```

## 3. Semantic class names

Semantic wrappers are appropriate when a repeated product concept would
otherwise carry a long utility list.

```css
@utility route-card-action {
  @apply inline-flex items-center gap-2 rounded-field px-3 py-2 text-sm font-medium;
  background: var(--color-primary);
  color: var(--color-primary-content);
}
```

Class names should describe product intent, not visual appearance. For example,
`route-card-action` is preferable to `blue-button`.

## 4. daisyUI with Tailwind utilities

daisyUI classes provide component structure. Tailwind utilities refine local
layout and state.

```html
<button class="btn btn-primary md:btn-lg shadow-sm">Create route</button>

<article class="card bg-base-100 shadow-sm">
  <div class="card-body">
    <h3 class="card-title">Hidden courtyards</h3>
  </div>
</article>
```

Raw palette utilities should be avoided in product markup where daisyUI role
classes can express the same intent.

## 5. Radix state styling

Radix state should be styled through `data-*` and ARIA variants:

```tsx
<Toggle.Root className="btn data-[state=on]:bg-primary data-[state=on]:text-primary-content">
  Hidden gems
</Toggle.Root>
```

Component classes such as `btn-primary` should be toggled in TypeScript when a
daisyUI variant changes by state. Tailwind variants should target Tailwind
utilities.

## 6. `@apply` and `@utility`

Use `@utility` for project wrappers that need variant support. Use `@apply` only
for Tailwind utilities.

```css
@utility poi-chip {
  @apply inline-flex items-center gap-2 rounded-field px-2 py-1 text-sm;
}

.poi-chip[data-state="selected"] {
  @apply ring-2 ring-primary;
}
```

Do not use `@apply` with daisyUI plugin component classes such as `btn` or
`card`. Compose those classes in markup or rebuild the required wrapper from
tokens.

## 7. Specificity and cascade

Selectors should remain low-specificity. State selectors belong near the wrapper
they modify, and broad overrides should be avoided.

## 8. Responsive and container behaviour

Use normal responsive utilities for page-level layout. Use container queries when
a component must adapt to the space allocated by its parent.

## 9. Checklist

- [ ] Semantic element first; ARIA only clarifies behaviour.
- [ ] daisyUI role colours and token-backed utilities are preferred over raw
      palette classes.
- [ ] Radix state is reflected through `data-*` or ARIA selectors.
- [ ] `@apply` is limited to Tailwind utilities.
- [ ] Repeated product concepts use small semantic wrappers.
- [ ] Tests query controls by accessible role, name, or label.

## 10. Tokens and daisyUI roles

Primitive token values belong in `packages/tokens/`. Semantic roles map those
values to daisyUI variables such as `--color-primary`, `--color-base-100` and
`--radius-field`.

Requirements:

- `--color-*-content` roles must meet contrast expectations against their paired
  background roles.
- Day and night themes should share role names even when values differ.
- Hover and focus states should derive from token roles, normally through
  `color-mix()` or generated token values.
- Token generation should produce CSS variables, Tailwind theme fragments and
  daisyUI role mappings from one source.
