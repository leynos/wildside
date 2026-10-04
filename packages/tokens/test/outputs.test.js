/** @file Hold the generated design-token files byte for byte.
 *
 * Style Dictionary turns `src/tokens.json` into CSS custom properties, a
 * Tailwind preset and a daisyUI theme, and the frontend imports all three. A
 * tool upgrade can change any of them without touching a line of source, so
 * each test rebuilds the outputs and compares them with a committed copy in
 * `test/golden` (named `.golden` so the formatter leaves Style Dictionary's own
 * formatting alone). A deliberate change to the tokens or the build is a reviewed
 * diff to those copies; an incidental change from a dependency bump is a
 * failure here instead of a surprise in the app.
 */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const packageRoot = fileURLToPath(new URL('..', import.meta.url));

/**
 * Read one file under the package root as text.
 *
 * @param {string} relativePath - Path relative to the package root.
 * @returns {string} The file contents.
 */
function read(relativePath) {
  return readFileSync(new URL(`../${relativePath}`, import.meta.url), 'utf8');
}

// Rebuild first, so the comparison never reads a stale `dist`.
execFileSync(process.execPath, ['build/style-dictionary.js'], {
  cwd: packageRoot,
  stdio: 'pipe',
});

/** Generated file and its committed copy, with a value that must appear. */
const outputs = [
  {
    name: 'CSS custom properties',
    built: 'dist/css/variables.css',
    golden: 'test/golden/variables.css.golden',
    mustContain: '--color-neutral-0: #FFFFFF;',
  },
  {
    name: 'Tailwind preset',
    built: 'dist/tw/preset.js',
    golden: 'test/golden/preset.js.golden',
    mustContain: 'export default',
  },
  {
    name: 'daisyUI theme',
    built: 'dist/daisy/theme.js',
    golden: 'test/golden/theme.js.golden',
    mustContain: '"themes"',
  },
];

for (const { name, built, golden, mustContain } of outputs) {
  test(`the ${name} output is unchanged`, () => {
    const actual = read(built);
    assert.ok(actual.includes(mustContain), `${built} must contain ${mustContain}`);
    assert.equal(
      actual,
      read(golden),
      `${built} differs from ${golden}; if the change is deliberate, update the copy`,
    );
  });
}
