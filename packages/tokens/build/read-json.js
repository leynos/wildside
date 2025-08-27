/** @file Utility helpers for working with JSON files in build scripts.
 * Provides a single function to synchronously read and parse a JSON file with
 * consistent error reporting. This avoids repeating boilerplate and ensures any
 * failures surface with clear context.
 */
import fs from 'node:fs';

/**
 * Read and parse a JSON file from disk.
 *
 * @param {string | URL} file - Path or URL pointing to the JSON file.
 * @returns {unknown} Parsed JSON content.
 * @throws {Error} When the file cannot be read or parsed.
 */
export function readJson(file) {
  try {
    const data = fs.readFileSync(file, 'utf8');
    return JSON.parse(data);
  } catch (err) {
    const fileHint = file instanceof URL ? file.pathname : file;
    console.error(`Failed to load JSON from ${fileHint}.`, err);
    throw err;
  }
}
