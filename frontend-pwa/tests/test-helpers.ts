/**
 * @file Shared test helpers for filesystem path handling.
 */
import type { PathLike } from 'node:fs';

/**
 * Coerces a PathLike into a string for easier assertions and comparisons.
 */
export function pathToString(path: PathLike): string {
  return typeof path === 'string' ? path : path.toString();
}
