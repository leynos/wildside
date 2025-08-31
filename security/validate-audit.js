/** @file Validate audit exception entries against schema and expiry. */
import schema from "./audit-exceptions.schema.json" with { type: "json" };
import data from "./audit-exceptions.json" with { type: "json" };

import Ajv from "ajv";
import addFormats from "ajv-formats";

const ajv = new Ajv({ allErrors: true });
addFormats(ajv); // enable "date" format validation

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
  const validate = ajv.compile(schema);
  if (!validate(entries)) {
    console.error(
      "Audit exceptions failed schema validation:",
      validate.errors,
    );
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
  if (expired.length > 0) {
    console.error("Audit exceptions have expired:");
    for (const { id, package: pkg, expiresAt } of expired) {
      console.error(`- ${id} (${pkg}) expired on ${expiresAt}`);
    }
    process.exit(1);
  }
}

assertValidSchema(data);
assertNoExpired(data);

console.log("Audit exceptions valid");
