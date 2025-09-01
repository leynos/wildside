/** @file Core logic for resolving design token references.
 *
 * Exposes `resolveToken` without a bundled token tree so environments can
 * supply their own structure. The wrapper in `tokens.js` wires the default
 * design tokens for Node consumers.
 */

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
 * @param {object} tokens - Token tree mirroring the structure of
 * `tokens.json`, where leaves contain a `value` string.
 * @returns {string} Token value.
 * @throws {TypeError} If `ref` is not a string or `tokens` is not an object.
 * @throws {Error} If the token path does not exist or a circular reference is detected.
 * @example
 * resolveToken('{color.brand}', { color: { brand: { value: '#fff' } } })
 */
function resolvePathOrThrow(tokens, key) {
  const segments = key.split('.');
  let cursor = tokens;
  for (const [index, segment] of enumerate(segments)) {
    const missing = segments.slice(0, index + 1).join('.');
    const hasObjectShape = cursor && typeof cursor === 'object';
    const siblings = hasObjectShape ? Object.keys(cursor).slice(0, 10) : [];
    const hint = siblings.length ? ` Available keys: ${siblings.join(', ')}` : '';

    // 1) Cursor is falsy
    if (!cursor) {
      throw new Error(
        `Token path "${missing}" not found (while resolving "${key}"). ` +
          `Reason: cursor is null/undefined.${hint}`,
      );
    }

    // 2) Cursor is not an object
    if (typeof cursor !== 'object') {
      throw new Error(
        `Token path "${missing}" not found (while resolving "${key}"). ` +
          `Reason: cursor is not an object.${hint}`,
      );
    }

    // 3) Segment missing on current object
    if (!Object.hasOwn(cursor, segment)) {
      throw new Error(`Token path "${missing}" not found (while resolving "${key}").${hint}`);
    }

    cursor = cursor[segment];
  }
  return cursor;
}

function getTokenValue(tokens, key) {
  const node = resolvePathOrThrow(tokens, key);
  const { value } = node ?? {};
  if (!hasTokenProperty(node, 'value') || !isValidTokenValue(value)) {
    throw new TypeError(`Token "${key}" must resolve to an object with a string "value"`);
  }
  return value;
}

/** Assert that the provided tokens tree is a valid object. */
function assertValidTokens(tokens) {
  if (!isValidTokenTree(tokens)) {
    throw new TypeError('tokens must be an object token tree');
  }
}

/**
 * Determine whether a tokens tree value is a non-null object.
 *
 * @param {unknown} value - Candidate tokens tree.
 * @returns {boolean} True when value is a valid object tree.
 */
function isValidTokenTree(value) {
  return value !== null && value !== undefined && typeof value === 'object';
}

/**
 * Safely check whether a cursor has a non-null/undefined property for a segment.
 *
 * This helper mirrors existing semantics that reject null/undefined leaf values
 * during traversal.
 *
 * @param {unknown} cursor - Current object in the token tree.
 * @param {string} segment - Key to check on the cursor.
 * @returns {boolean} True when the property exists and is not null/undefined.
 */
function hasTokenProperty(cursor, segment) {
  return cursor?.[segment] !== null && cursor?.[segment] !== undefined;
}

/**
 * Validate that a resolved token value is a present string.
 *
 * @param {unknown} value - Resolved token leaf value.
 * @returns {boolean} True when value is a non-null string.
 */
function isValidTokenValue(value) {
  return value !== null && value !== undefined && typeof value === 'string';
}

export function resolveToken(ref, tokens) {
  if (typeof ref !== 'string') {
    throw new TypeError('ref must be a string like "{path.to.token}" or a literal string');
  }
  assertValidTokens(tokens);

  const seen = new Set();
  let current = ref;
  const refRe = /^\{(.+)\}$/;

  while (typeof current === 'string') {
    const match = refRe.exec(current.trim());
    if (!match) return current;

    const key = match[1].trim();
    if (seen.has(key)) throw new Error(`Circular token reference detected: "${key}"`);
    seen.add(key);
    current = getTokenValue(tokens, key);
  }
  return current;
}
