/**
 * @file Shared direct-execution detection for repository CLI scripts.
 *
 * Both the dependency-override policy check and the bun audit runner guard
 * their top-level side effects behind an "am I the entry point?" test. The
 * comparison resolves symlinks so a symlinked launch (for example a
 * `node_modules/.bin` shim) still matches the real script path.
 */

import { realpathSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Resolves a filesystem path through symlinks, preferring the native
 * implementation when available.
 *
 * @param {string} path - The path to normalize.
 * @returns {string} The canonical, symlink-resolved path.
 */
export const normalize = (path) =>
  typeof realpathSync.native === 'function' ? realpathSync.native(path) : realpathSync(path);

/**
 * Reports whether the given module metadata identifies the process entry point.
 *
 * @param {ImportMeta} meta - The importing module's `import.meta`.
 * @returns {boolean} True when the module is the directly executed script.
 */
export function isExecutedDirectly(meta) {
  const invokedPath = process.argv?.[1];
  if (!invokedPath) {
    return false;
  }

  try {
    const scriptPath = fileURLToPath(meta.url);
    return normalize(scriptPath) === normalize(resolve(invokedPath));
  } catch {
    return false;
  }
}
