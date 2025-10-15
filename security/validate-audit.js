/** @file Validate audit exception entries against schema and expiry. */
import { spawnSync } from 'node:child_process';
import Ajv from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';
import { isValidatorPatched } from './validator-patch.js';

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

function runAuditJson() {
  const result = spawnSync('pnpm', ['audit', '--json'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
  });

  if (result.error) {
    throw result.error;
  }

  if (!result.stdout) {
    return { advisories: {} };
  }

  try {
    return JSON.parse(result.stdout);
  } catch (error) {
    error.message = `Failed to parse pnpm audit JSON: ${error.message}`;
    throw error;
  }
}

function assertMitigated(entries, advisories) {
  if (advisories.length === 0) {
    return;
  }

  const exceptionsById = new Map(entries.map((entry) => [entry.advisory, entry]));
  const unexpected = [];

  for (const advisory of advisories) {
    const exception = exceptionsById.get(advisory.github_advisory_id);
    if (!exception) {
      unexpected.push(advisory);
      continue;
    }

    if (
      advisory.github_advisory_id === 'GHSA-9965-vmph-33xx' &&
      !isValidatorPatched()
    ) {
      console.error(
        'Validator vulnerability GHSA-9965-vmph-33xx reported but local patch is missing.',
      );
      process.exit(1);
    }
  }

  if (unexpected.length > 0) {
    console.error('pnpm audit reported vulnerabilities without exceptions:');
    for (const advisory of unexpected) {
      console.error(`- ${advisory.github_advisory_id}: ${advisory.title}`);
    }
    process.exit(1);
  }
}

assertValidSchema(data);
assertNoExpired(data);

const auditJson = runAuditJson();
const advisories = Object.values(auditJson.advisories ?? {});
assertMitigated(data, advisories);

console.log('Audit exceptions valid and vulnerabilities accounted for');
