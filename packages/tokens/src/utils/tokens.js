/** @file Token utilities with the bundled token tree.
 *
 * Loads the design tokens JSON once and wires it to `resolveToken`. A lean
 * browser entry is available via the conditional export `"browser"` which
 * omits this import to avoid bundling unused data.
 */
import tokensJson from '../tokens.json' assert { type: 'json' };
import { resolveToken as baseResolveToken } from './resolve-token.js';

// Freeze to guard against accidental mutation at runtime.
const TOKENS = Object.freeze(tokensJson);

/**
 * Resolve a `{token.path}` reference to its concrete value.
 * Follows chained references and detects cycles.
 *
 * @param {string} ref - Token reference in `{path.to.token}` form.
 * @param {object} [tokens=TOKENS] - Token tree mirroring the structure of
 * `tokens.json`, where leaves contain a `value` string.
 * @returns {string} Token value.
 * @throws {TypeError} If `ref` is not a string or `tokens` is not an object.
 * @throws {Error} If the token path does not exist or a circular reference is detected.
 * @example
 * resolveToken('{color.brand}')
 * resolveToken('{color.brand}', { color: { brand: { value: '#fff' } } })
 */
export function resolveToken(ref, tokens = TOKENS) {
  return baseResolveToken(ref, tokens);
}

