# High-velocity, accessibility-first component testing

This document defines the Wildside front-end accessibility test strategy. It is
project-specific guidance for keeping accessible behaviour in the fast feedback
loop while still using real-browser checks for layout, colour contrast and
assistive-technology-facing output.

## I. Strategic framework: layered accessibility checks

The test architecture has three layers:

1. **Fast component tests:** Bun or the default front-end test runner verifies
   component rendering, accessible names, state transitions and user-centred
   queries.
2. **Focused accessibility scans:** a Node-compatible JSDOM/Vitest harness runs
   `axe-core` for component states where static semantic checks are useful.
3. **Real-browser checks:** Playwright covers keyboard navigation, focus
   management, route announcements, colour contrast, screenshots and
   accessibility-tree snapshots.

This split keeps the inner loop fast without pretending that a headless DOM can
verify every accessibility property. JSDOM is useful for semantic structure;
Playwright is required for rendered layout, browser focus behaviour, colour
contrast and canvas-adjacent UI.

## II. Component and accessibility harnesses

### 1.1 Fast component tests

Component tests should use Testing Library queries by role, label, text or other
user-visible semantics. Ordinary controls should not require `data-testid`.

Required coverage for reusable components:

- accessible name and role for the primary interactive element,
- keyboard-operable state changes,
- disabled, loading and error states,
- localization-sensitive labels or formatted values, and
- focus behaviour where the component owns focus movement.

### 1.2 Node and JSDOM accessibility scans

A parallel Node.js test harness is used for `axe-core` scans. These tests should
live in dedicated `*.a11y.test.tsx` files so that accessibility scans remain
discoverable and can run independently when needed.

JSDOM limitations must be explicit. Rules that depend on layout, CSS pixels or
browser rendering, such as `color-contrast`, should be disabled in component
scans and covered in Playwright instead. Each disabled rule needs a short note
describing the real-browser test that covers the same risk.

### 1.3 Snapshot guard

Snapshot tests are allowed only when the output is stable and meaningful.
Unchecked snapshot updates should fail the test run through the snapshot guard
imported from the v2a mockup test setup.

## III. Playwright and browser-level checks

Playwright is the outer loop for behaviour that only a browser can validate.

Required smoke coverage:

- route loading and route-change announcements,
- skip link and main landmark focus,
- keyboard navigation through menus, tabs, dialogs and forms,
- modal focus trap and focus return,
- colour contrast and visible focus checks,
- responsive layouts at mobile and wider viewports,
- day and night theme screenshots for critical screens,
- accessibility-tree snapshots for composite widgets, and
- at least one non-default locale for each registry-driven feature surface.

### 3.2.1 Keyboard navigation flow tests

All functionality should be operable by keyboard. E2E tests should exercise
`Tab`, `Shift+Tab`, `Enter`, `Space` and `Escape` where relevant. Assertions
should verify both DOM state and visible focus movement.

### 3.2.2 Focus management tests

Dialogs, popovers, drawers and route transitions need explicit focus assertions.
The expected behaviour is:

- opening a modal moves focus into the modal,
- closing a modal returns focus to the trigger or a documented fallback,
- route changes focus the main heading or main landmark after navigation, and
- no keyboard trap exists outside components that intentionally contain focus.

### 3.2.3 Localization and direction tests

Locale tests should validate document `lang`, `dir`, translated UI chrome,
entity localization fallback and long-string layout resilience. Right-to-left
coverage should be added when an RTL locale enters the supported locale set.

## IV. Semantic test conventions

Test code should reinforce accessibility expectations:

- Prefer `userEvent` or Playwright user actions over direct event dispatch.
- Prefer `getByRole`, `findByRole`, `getByLabelText` and accessible text
  queries.
- Avoid `querySelector`, class selectors and test IDs unless semantics are
  genuinely unavailable.
- Add assertions for focus, ARIA state or live-region output when a component
  owns those behaviours.

The semantic lint rules imported from the v2a mockup should enforce these
conventions where tooling can express them reliably.

## V. Continuous integration and reporting

CI should keep the layers separate enough to diagnose failures quickly:

- component tests and accessibility scans run as fast pull-request gates,
- Playwright smoke tests run against a built or previewed front-end,
- browser traces, screenshots and accessibility reports are uploaded for failed
  runs, and
- severe `axe-core` violations fail the gate.

Longer Playwright suites may run outside the fastest pull-request gate when the
core smoke suite already covers the release-blocking flows.

## VI. Adoption checklist

- [ ] Component tests use accessible queries for ordinary controls.
- [ ] `*.a11y.test.tsx` scans cover shared primitives and key states.
- [ ] JSDOM-disabled accessibility rules have matching Playwright coverage.
- [ ] Playwright covers keyboard navigation, focus management and route
      announcements.
- [ ] Theme and viewport checks catch colour contrast and responsive regressions.
- [ ] Localization checks cover at least one non-default locale for each major
      registry-driven surface.
- [ ] Snapshot updates require deliberate review.

## References

- `docs/v2a-front-end-stack.md`
- `docs/wildside-pwa-design.md`
- `docs/enforcing-semantic-tailwind-best-practice.md`
- `docs/semantic-tailwind-with-daisyui-best-practice.md`
