# daisyUI v5 project guide

This guide records Wildside-specific daisyUI v5 usage. It intentionally avoids a
full copy of the upstream component reference. Use the official daisyUI
documentation for exhaustive component markup, modifier lists and browser
support details: <https://daisyui.com/docs/>.

## daisyUI 5 install notes

Wildside should load daisyUI through Tailwind v4 CSS configuration:

```css
@import "tailwindcss";

@plugin "daisyui" {
  themes:
    wildside-day --default,
    wildside-night --prefersdark;
}
```

Implementation expectations:

- daisyUI roles are generated from the repository token pipeline rather than
  copied from the mockup by hand.
- Theme names, role mappings and contrast checks are documented beside
  `packages/tokens/`.
- Component markup follows semantic HTML first; daisyUI classes provide
  presentation, not semantics.
- CDN usage is out of scope for production Wildside builds.

## daisyUI 5 usage rules

1. Styles are applied to an HTML element by combining daisyUI component classes,
   optional part classes and optional modifier classes.
2. Tailwind utilities may refine local spacing, layout and state when existing
   daisyUI modifiers are not sufficient.
3. Forced utility overrides such as `bg-red-500!` should be avoided except for a
   narrowly documented compatibility case.
4. Product-specific repeated patterns belong in semantic wrappers or token roles,
   not long duplicated class lists.
5. Placeholder imagery is only suitable for mockups and must not ship in product
   routes.

## daisyUI colour roles

daisyUI exposes semantic colour role names that map to CSS custom properties.
Wildside should use role names in markup and keep raw palette choices inside the
token source.

Primary roles:

- `primary` and `primary-content`: main brand action colour and readable
  foreground.
- `secondary` and `secondary-content`: secondary brand action colour and
  readable foreground.
- `accent` and `accent-content`: accent colour for secondary emphasis.
- `neutral` and `neutral-content`: neutral UI surfaces and foreground.
- `base-100`, `base-200`, `base-300`, and `base-content`: page and surface
  layers.
- `info`, `success`, `warning`, `error`, and matching `*-content`: status
  colours and readable foregrounds.

### daisyUI colour rules

1. daisyUI adds semantic colour names to Tailwind CSS utilities.
2. daisyUI colour names can be used in utility classes like other Tailwind CSS
   colour names. For example, `bg-primary` uses the primary role for the active
   theme.
3. daisyUI colour names resolve through variables, so they can change by theme.
4. `dark:` variants are not needed for daisyUI role colours.
5. Tailwind palette utilities such as `text-gray-800` should be avoided in
   product markup because they do not automatically adapt across day and night
   themes.
6. `*-content` roles must maintain accessible contrast against their associated
   background roles.
7. Page surfaces should normally use `base-*` roles, while important actions use
   `primary` or another semantic action role.

## Custom theme skeleton

The theme skeleton below shows the role names expected by daisyUI. Values should
come from the Wildside token source.

```css
@plugin "daisyui/theme" {
  name: "wildside-day";
  default: true;
  prefersdark: false;
  color-scheme: light;

  --color-base-100: oklch(98% 0.02 240);
  --color-base-200: oklch(95% 0.03 240);
  --color-base-300: oklch(92% 0.04 240);
  --color-base-content: oklch(20% 0.05 240);
  --color-primary: oklch(55% 0.3 240);
  --color-primary-content: oklch(98% 0.01 240);

  --radius-selector: 0.5rem;
  --radius-field: 0.25rem;
  --radius-box: 0.5rem;

  --size-selector: 0.25rem; /* Keep at 0.25rem unless a larger selector is intentional. If so, use 0.28125rem or 0.3125rem. */
  --size-field: 0.25rem; /* Keep at 0.25rem unless a larger field is intentional. If so, use 0.28125rem or 0.3125rem. */

  --border: 1px;
  --depth: 1;
  --noise: 0;
}
```

## Component usage boundary

Component-specific markup should come from upstream daisyUI documentation at the
time of implementation. Repository documentation should only record Wildside
policy, local exceptions and integration notes that are not obvious from the
upstream reference.
