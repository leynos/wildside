# Enforcing semantic Tailwind best practice

**Audience:** Wildside front-end implementers and tooling maintainers.
**Goal:** enforce semantic, accessible HTML with clean, token-driven Tailwind
and daisyUI usage.
**Outcome:** readable markup, reusable semantic wrappers, predictable Radix
state styling and executable lint gates.

## 1. Scope

The enforcement stack covers `.tsx`, `.html` and `.css` files that use Tailwind
CSS v4, daisyUI v5, Radix primitives and generated Wildside tokens.

Primary tools:

- Biome for standard formatting and linting.
- GritQL rules for semantic JSX/HTML patterns.
- Semgrep for simple cross-file structural checks.
- Stylelint for CSS token, colour and selector policy.
- Custom scripts for class-list length, near-duplicate classes and Fluent
  variable usage.

## 2. Design doctrine

1. Semantic elements come first.
2. ARIA clarifies behaviour; it does not replace native elements.
3. daisyUI component classes provide structure and theme-aware presentation.
4. Tailwind utilities refine local layout and state.
5. Repeated utility stacks move into semantic wrappers.
6. Raw hex, named colours and raw Tailwind palette classes are discouraged in
   product markup and CSS.

## 3. Rule inventory

The v2a mockup provides the initial rule set:

- `tools/grit/rule-a11y*.grit`: clickable non-interactive elements.
- `tools/grit/rule-daisyui-*.grit`: daisyUI component class misuse.
- `tools/grit/rule-landmark-slot.grit`: semantic landmarks.
- `tools/grit/rule-state-slot-*.grit`: Radix and ARIA state slots.
- `tools/grit/rule-heading-semantic-*.grit`: heading structure.
- `tools/grit/rule-layout-wrapper-*.grit`: layout wrapper hints.
- `tools/grit/rule-testing-*.grit`: test selector and user-event policy.
- `tools/semgrep-semantic.yml`: raw utility and token misuse checks.
- `tools/stylelint.config.cjs`: CSS colour, token and selector checks.

## 4. Biome and GritQL integration

Grit rules should stay granular while Biome plugin support expects one useful
pattern per file. Rules should emit actionable diagnostics that name the semantic
replacement, for example replacing a clickable `<div>` with `<button>`.

## 5. Accessibility rules

Accessibility rules should catch:

- `onClick` on non-interactive elements,
- missing accessible names on interactive controls,
- avoidable ARIA role substitutions for native elements,
- missing landmark structure, and
- invalid heading jumps in local component context.

## 6. daisyUI rules

daisyUI rules should prevent component classes from being applied to elements
with incompatible semantics. Examples include `btn` on `<div>` or `input` on
non-form elements.

## 7. State-slot rules

Radix and ARIA state should be visible in markup through `data-state`,
`aria-selected`, `aria-current`, `role="tab"` and related attributes. State that
only exists in a class string is harder to test and should be flagged where a
semantic attribute is available.

## 8. Class-list and wrapper rules

Long repeated class lists should either become a component, a semantic wrapper or
a documented exception. Near-duplicate class sets should warn before they become
copy-and-paste styling debt.

## 9. Testing-selector rules

Test code should prefer role, label and text queries. `data-testid`,
`querySelector`, class selectors and text-click shortcuts should require a local
exception when ordinary semantics are available.

## 10. CSS token and colour rules

Stylelint should enforce:

- no raw hex colours in product CSS,
- no named colours in product CSS,
- token-backed custom properties for theme values,
- low selector specificity, and
- no `@apply` of daisyUI plugin component classes.

Code identifiers such as `--color-primary`, `color-mix()` and Stylelint rule
names keep their upstream spelling.

## 11. Adoption checklist

- [ ] Import the v2a mockup rule sources into this repository.
- [ ] Document each imported rule and its owning policy source.
- [ ] Wire enforced checks into Makefile targets.
- [ ] Mark advisory checks explicitly when they are not gate-worthy yet.
- [ ] Add fixture examples for each custom rule family.
- [ ] Keep allowlists narrow, named and reviewed.
