/** @file Browser entry for token utilities without the bundled JSON tree. */
import { resolveToken as baseResolveToken } from './resolve-token.js';

/**
 * Default token tree for browser builds is intentionally undefined.
 * Callers must supply a token object at runtime in browser environments.
 */
export const DefaultTokens = undefined;

/**
 * Resolve a `{token.path}` reference using an injected token tree.
 * Mirrors the Node entry's signature so callers can omit the second argument
 * and receive a clear `TypeError` when tokens are not provided.
 *
 * @param {string} ref - Token reference in `{path.to.token}` form.
 * @param {object|undefined} [tokens=DefaultTokens] - Token tree mirroring `tokens.json`.
 * @returns {string|undefined} Token value.
 * @throws {TypeError} If `ref` is not a string or `tokens` is not an object.
 * @throws {Error} If the token path does not exist or a circular reference is detected.
 */
export function resolveToken(ref, tokens = DefaultTokens) {
  return baseResolveToken(ref, tokens);
}
