/** @file Token utilities with the bundled token tree.
 *
 * Loads the design tokens JSON once and wires it to `resolveToken`.
 *
 * Note: We intentionally avoid `import ... assert { type: 'json' }` here
 * because some tooling (formatters/linters) in this repository does not yet
 * parse import assertions. Since this file is the Node entry (the browser
 * entry is provided via the `browser` export), it is safe to read the JSON
 * using Node's FS API instead.
 */
import { readFileSync } from 'node:fs';
import { resolveToken as baseResolveToken } from './resolve-token.js';

// Load and parse the token JSON at module initialisation.
// Using URL ensures correctness regardless of CWD.
const tokensJson = JSON.parse(readFileSync(new URL('../tokens.json', import.meta.url), 'utf8'));

// Freeze to guard against accidental mutation at runtime.
const DefaultTokens = Object.freeze(tokensJson);

/**
 * Resolve a `{token.path}` reference to its concrete value.
 * Follows chained references and detects cycles.
 *
 * @param {string} ref - Token reference in `{path.to.token}` form.
 * @param {object} [tokens=DefaultTokens] - Token tree mirroring the structure of
 * `tokens.json`, where leaves contain a `value` string.
 * @returns {string} Token value.
 * @throws {TypeError} If `ref` is not a string or `tokens` is not an object.
 * @throws {Error} If the token path does not exist or a circular reference is detected.
 * @example
 * resolveToken('{color.brand}')
 * resolveToken('{color.brand}', { color: { brand: { value: '#fff' } } })
 */
export function resolveToken(ref, tokens = DefaultTokens) {
  return baseResolveToken(ref, tokens);
}
