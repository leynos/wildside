/** @file Validate audit exception entries against schema and expiry. */
import Ajv from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';

/**
 * Load a JSON file using the import attribute supported by the current Node
 * version.
 *
 * Node 18 expects `assert { type: 'json' }` while Node ≥20.6 uses
 * `with { type: 'json' }`.
 *
 * @param {string} relPath Path to the JSON module.
 * @returns {Promise<unknown>} Parsed JSON contents.
 */
async function importJson(relPath) {
  const [major, minor] = process.versions.node.split('.').map(Number);
  const attrKey = major >= 20 && !(major === 20 && minor < 6) ? 'with' : 'assert';
  return (await import(relPath, { [attrKey]: { type: 'json' } })).default;
}

const schema = await importJson('./audit-exceptions.schema.json');
const data = await importJson('./audit-exceptions.json');

const ajv = new Ajv({ allErrors: true });
addFormats(ajv); // enable "date" format validation
const validate = ajv.compile(schema);

/**
 * Validate audit exceptions against the JSON Schema.
 *
 * @param {typeof data} entries Entries to validate.
 * @example
 * assertValidSchema([
 *   {
 *     id: "1",
 *     package: "pkg",
 *     advisory: "ADV-1",
 *     reason: "Justified",
 *     addedAt: "2024-01-01",
 *     expiresAt: "2099-01-01",
 *   },
 * ]);
 */
function assertValidSchema(entries) {
  if (!validate(entries)) {
    console.error('Audit exceptions failed schema validation:', validate.errors);
    process.exit(1);
  }
}

/**
 * Exit with error if any audit exceptions are past their expiry date.
 *
 * @param {typeof data} entries Entries to inspect.
 * @example
 * assertNoExpired([
 *   {
 *     id: "1",
 *     package: "pkg",
 *     advisory: "ADV-1",
 *     reason: "Justified",
 *     addedAt: "2024-01-01",
 *     expiresAt: "2099-01-01",
 *   },
 * ]);
 */
function assertNoExpired(entries) {
  const today = new Date().toISOString().slice(0, 10);
  const expired = entries.filter((e) => e.expiresAt < today);
  const inverted = entries.filter((e) => e.addedAt > e.expiresAt);
  if (expired.length > 0) {
    console.error('Audit exceptions have expired:');
    for (const { id, package: pkg, expiresAt } of expired) {
      console.error(`- ${id} (${pkg}) expired on ${expiresAt}`);
    }
    process.exit(1);
  }
  if (inverted.length > 0) {
    console.error('Audit exceptions have invalid date ranges (addedAt > expiresAt):');
    for (const { id, package: pkg, addedAt, expiresAt } of inverted) {
      console.error(`- ${id} (${pkg}) addedAt ${addedAt} > expiresAt ${expiresAt}`);
    }
    process.exit(1);
  }
}

assertValidSchema(data);
assertNoExpired(data);

console.log('Audit exceptions valid');
