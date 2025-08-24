/** @file Browser entry for token utilities without the bundled JSON tree. */
import { resolveToken as baseResolveToken } from './resolve-token.js';

// No default token tree to avoid bundling large JSON payloads in the browser.
export const TOKENS = undefined;

/**
 * Resolve a `{token.path}` reference using an injected token tree.
 * Mirrors the Node entry's signature so callers can omit the second argument
 * and receive a clear `TypeError` when tokens are not provided.
 *
 * @param {string} ref - Token reference in `{path.to.token}` form.
 * @param {object} [tokens=TOKENS] - Token tree mirroring `tokens.json`.
 * @returns {string} Token value.
 * @throws {TypeError} If `ref` is not a string or `tokens` is not an object.
 * @throws {Error} If the token path does not exist or a circular reference is detected.
 */
export function resolveToken(ref, tokens = TOKENS) {
  return baseResolveToken(ref, tokens);
}


