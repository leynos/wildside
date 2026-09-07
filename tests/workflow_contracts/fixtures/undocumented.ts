// Fixture for the TypeDoc documentation gate's contract tests.
//
// `documentation_gate_test.py` runs TypeDoc over this file with the fixture
// configuration beside it and asserts the run fails, naming the export below.
//
// These are line comments on purpose. A `/** ... */` block here would attach
// to the declaration as its documentation, and the export must stay
// undocumented or the test it serves passes for the wrong reason.

export function undocumentedFixture(): void {}
