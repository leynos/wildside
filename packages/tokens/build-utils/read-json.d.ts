/**
 * Read and parse a JSON file from disk.
 *
 * @template T
 * @param {string | URL} file Path or URL pointing to the JSON file.
 * @returns {T} Parsed JSON content.
 * @throws {Error} When the file cannot be read or parsed.
 */
export function readJson<T>(file: string | URL): T;
