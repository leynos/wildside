/** @file Token utilities for resolving design token references. */
import fs from 'node:fs';

// Load token tree once for reference resolution.
const TOKENS = JSON.parse(
  fs.readFileSync(new URL('../tokens.json', import.meta.url), 'utf8'),
);

/**
 * Resolve a `{token.path}` reference to its concrete value.
 * Follows chained references and detects cycles.
 *
 * @param {string} ref - Token reference in `{path.to.token}` form.
 * @returns {string} Token value.
 * @example
 * resolveToken('{color.brand}')
 */
export function resolveToken(ref, tokens = TOKENS) {
  let current = ref;
  const seen = new Set();
  while (typeof current === 'string') {
    const match = /^\{(.+)\}$/.exec(current.trim());
    if (!match) return current;
    const key = match[1];
    if (seen.has(key)) {
      throw new Error(`Circular token reference detected: "${key}"`);
    }
    seen.add(key);
    const pathSegments = key.split('.');
    let cursor = tokens;
    for (let segmentIndex = 0; segmentIndex < pathSegments.length; segmentIndex++) {
      const segment = pathSegments[segmentIndex];
      if (cursor?.[segment] == null) {
        const missingPath = pathSegments.slice(0, segmentIndex + 1).join('.');
        throw new Error(
          `Token path "${missingPath}" not found (while resolving "${key}")`,
        );
      }
      cursor = cursor[segment];
    }
    current = cursor?.value;
  }
  return current;
}
