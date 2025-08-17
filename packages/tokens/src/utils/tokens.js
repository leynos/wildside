/** @file Token utilities for resolving design token references. */
import fs from 'node:fs';

// Load token tree once for reference resolution.
const TOKENS = JSON.parse(
  fs.readFileSync(new URL('../tokens.json', import.meta.url), 'utf8')
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
export function resolveToken(ref) {
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
    let obj = TOKENS;
    for (let i = 0; i < pathSegments.length; i++) {
      const k = pathSegments[i];
      if (obj?.[k] == null) {
        const missingPath = pathSegments.slice(0, i + 1).join('.');
        throw new Error(`Token path "${missingPath}" not found (while resolving "${key}")`);
      }
      obj = obj[k];
    }
    current = obj?.value;
  }
  return current;
}
