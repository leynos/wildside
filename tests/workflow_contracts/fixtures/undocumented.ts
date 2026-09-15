/**
 * @module
 * Fixture whose only export is deliberately undocumented.
 *
 * `documentation_gate_test.py` runs TypeDoc over this file with the fixture
 * configuration beside it and asserts the run fails, naming the export below.
 *
 * The tag is `@module` rather than the `@file` the rest of the repository
 * uses. Under this fixture's configuration `@file` is an unknown block tag,
 * and the block is then read as the sole declaration's own documentation:
 * TypeDoc reports one unknown-tag warning, no `notDocumented` error, and the
 * contract passes while proving nothing. `@module` binds the comment to the
 * module, so the export stays undocumented and the run still fails.
 */

// The export itself carries no documentation, and must not. A `/** ... */`
// block immediately above the declaration would attach to it, which is the
// same vacuous pass by another route.

export function undocumentedFixture(): void {}
