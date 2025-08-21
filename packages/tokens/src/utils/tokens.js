/** @file Token utilities for resolving design token references. */
import fs from 'node:fs';

// Load token tree once for reference resolution.
const TOKENS = JSON.parse(
  fs.readFileSync(new URL('../tokens.json', import.meta.url), 'utf8'),
);

/**
 * Iterate over `iterable` yielding `[index, value]` pairs.
 *
 * @template T
 * @param {Iterable<T>} iterable - Sequence to walk.
 * @returns {IterableIterator<[number, T]>}
 * @example
 * for (const [i, v] of enumerate(['a', 'b'])) {
 *   console.log(i, v);
 * }
 */
function* enumerate(iterable) {
  let index = 0;
  for (const value of iterable) {
    yield [index++, value];
  }
}

/**
 * Resolve a `{token.path}` reference to its concrete value.
 * Follows chained references and detects cycles.
 *
 * @param {string} ref - Token reference in `{path.to.token}` form.
 * @param {object} [tokens=TOKENS] - Token tree mirroring the structure of
 * `tokens.json`, where leaves contain a `value` string.
 * @returns {string} Token value.
 * @example
 * resolveToken('{color.brand}')
 * resolveToken('{color.brand}', { color: { brand: { value: '#fff' } } })
 */
export function resolveToken(ref, tokens = TOKENS) {
  if (tokens == null || typeof tokens !== 'object') {
    throw new TypeError('tokens must be an object token tree');
  }
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
    for (const [segmentIndex, segment] of enumerate(pathSegments)) {
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
