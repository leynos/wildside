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
    cursor = validateCursor(cursor, segment, segments, index, key);
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
 * Create an error for a missing token path with optional reason and sibling hint.
 *
 * Preserves original wording and formatting used throughout token resolution.
 *
 * @param {string} missing - Portion of the path that could not be found.
 * @param {string} fullKey - Full token key being resolved.
 * @param {string | null} reason - Optional reason to include after the base message.
 * @param {unknown} cursor - Current cursor to derive sibling keys for hinting.
 * @returns {Error} Error instance with a detailed message.
 */
function createMissingPathError(missing, fullKey, reason, cursor) {
  const hasObjectShape = cursor && typeof cursor === 'object';
  const siblings = hasObjectShape ? Object.keys(cursor).slice(0, 10) : [];
  const hint = siblings.length ? ` Available keys: ${siblings.join(', ')}` : '';
  const base = `Token path "${missing}" not found (while resolving "${fullKey}").`;
  return reason ? new Error(`${base} Reason: ${reason}.${hint}`) : new Error(`${base}${hint}`);
}

/**
 * Validate the traversal cursor and return the next cursor.
 *
 * - Throws when cursor is null/undefined.
 * - Throws when cursor is not an object.
 * - Throws when the segment does not exist on the cursor.
 * - Otherwise returns the next cursor value.
 *
 * Error messages and hints match the original implementation exactly.
 *
 * @param {unknown} cursor - Current traversal object.
 * @param {string} segment - Path segment to access.
 * @param {string[]} segments - All path segments.
 * @param {number} index - Index of the current segment.
 * @param {string} fullKey - Full token key being resolved.
 * @returns {unknown} The next cursor value.
 */
function validateCursor(cursor, segment, segments, index, fullKey) {
  const missing = segments.slice(0, index + 1).join('.');
  if (!cursor) {
    throw createMissingPathError(missing, fullKey, 'cursor is null/undefined', cursor);
  }
  if (typeof cursor !== 'object') {
    throw createMissingPathError(missing, fullKey, 'cursor is not an object', cursor);
  }
  if (!Object.hasOwn(cursor, segment)) {
    throw createMissingPathError(missing, fullKey, null, cursor);
  }
  return cursor[segment];
}

/**
 * Process a potential token reference string and advance resolution.
 *
 * Returns `{ resolved: true, value }` when the value is a non-braced string
 * (i.e., a literal) or a non-string value (early return). Returns
 * `{ resolved: false, value }` when a token reference `{...}` was parsed and
 * dereferenced to the next value.
 *
 * Throws on circular references using the same error message as the original.
 *
 * @param {unknown} current - Current value being processed.
 * @param {object} tokens - Token tree for dereferencing.
 * @param {Set<string>} seen - Set of seen keys for cycle detection.
 * @returns {{ resolved: boolean, value: unknown }} Step result.
 */
function processTokenReference(current, tokens, seen) {
  if (typeof current !== 'string') return { resolved: true, value: current };
  const refRe = /^\{(.+)\}$/;
  const match = refRe.exec(current.trim());
  if (!match) return { resolved: true, value: current };

  const key = match[1].trim();
  if (seen.has(key)) throw new Error(`Circular token reference detected: "${key}"`);
  seen.add(key);
  const next = getTokenValue(tokens, key);
  return { resolved: false, value: next };
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
  // Iterate token references until a literal string is reached.
  // Behaviour and error messages mirror the original implementation.
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const step = processTokenReference(current, tokens, seen);
    if (step.resolved) return step.value;
    current = step.value;
  }
}
