/** @file Typed declaration for the readJson utility used by token build scripts.
 * Read and parse a JSON file from disk synchronously; throw with cause on failure.
 *
 * @template T = unknown
 * @param {string | URL} file Path or URL pointing to the JSON file.
 * @returns {T} Parsed JSON content.
 * @throws {Error} When the file cannot be read or parsed.
 * @example
 * // TypeScript
 * import { readJson } from '../build-utils/read-json.js';
 * type Pkg = { name: string };
 * const pkg = readJson<Pkg>(new URL('../../package.json', import.meta.url));
 */
export function readJson<T = unknown>(file: string | URL): T;
