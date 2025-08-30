/** @file Validate audit exception entries against schema and expiry. */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import Ajv from "ajv";
import addFormats from "ajv-formats";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const schema = JSON.parse(
  fs.readFileSync(path.join(__dirname, "audit-exceptions.schema.json"), "utf8"),
);

const data = JSON.parse(
  fs.readFileSync(path.join(__dirname, "audit-exceptions.json"), "utf8"),
);

const ajv = new Ajv({ allErrors: true });
addFormats(ajv); // enable "date" format validation

const validate = ajv.compile(schema);
if (!validate(data)) {
  console.error("Audit exceptions failed schema validation:");
  console.error(validate.errors);
  process.exit(1);
}

// Compare using ISO strings to avoid time zone concerns.
const today = new Date().toISOString().slice(0, 10);
const expired = data.filter((entry) => entry.expiresAt < today);

if (expired.length > 0) {
  console.error("Audit exceptions have expired:");
  for (const entry of expired) {
    console.error(`- ${entry.id} (${entry.package}) expired on ${entry.expiresAt}`);
  }
  process.exit(1);
}

console.log("Audit exceptions valid");
