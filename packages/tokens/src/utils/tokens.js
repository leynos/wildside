/** @file Token utilities for resolving design token references. */
import fs from 'node:fs';

/**
 * Enumerate an array yielding [index, value] pairs.
 *
 * @template T
 * @param {T[]} array - Array to iterate.
 * @returns {IterableIterator<[number, T]>}
 */
function* enumerate(array) {
  for (let i = 0; i < array.length; i++) {
    yield [i, array[i]];
  }
}

// Load token tree once for reference resolution.
const TOKENS = JSON.parse(fs.readFileSync(new URL('../tokens.json', import.meta.url), 'utf8'));

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
    let cursor = TOKENS;
    for (const [segmentIndex, segment] of enumerate(pathSegments)) {
      if (cursor?.[segment] == null) {
        const missingPath = pathSegments.slice(0, segmentIndex + 1).join('.');
        throw new Error(`Token path "${missingPath}" not found (while resolving "${key}")`);
      }
      cursor = cursor[segment];
    }
    current = cursor?.value;
  }
  return current;
}
