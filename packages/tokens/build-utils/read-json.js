/** @file Utility helpers for working with JSON files in build scripts.
 * Provides a single function to synchronously read and parse a JSON file with
 * consistent error reporting. This avoids repeating boilerplate and ensures any
 * failures surface with clear context.
 */
import fs from 'node:fs';

/**
 * Read and parse a JSON file from disk.
 *
 * @template T
 * @param {string | URL} file - Path or URL pointing to the JSON file.
 * @returns {T} Parsed JSON content.
 * @throws {Error} When the file cannot be read or parsed.
 * @example
 * ```js
 * // Load package metadata
 * const pkg = readJson(new URL('./package.json', import.meta.url));
 * console.log(pkg.name);
 * ```
 */
export function readJson(file) {
  try {
    const data = fs.readFileSync(file, 'utf8');
    // Some editors prefix files with a UTF-8 BOM; JSON.parse rejects it.
    const text = data.charCodeAt(0) === 0xfeff ? data.slice(1) : data;
    return JSON.parse(text);
  } catch (err) {
    const fileHint = file instanceof URL ? file.pathname : file;
    throw new Error(`Failed to load JSON from ${fileHint}`, { cause: err });
  }
}
