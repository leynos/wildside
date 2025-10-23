/** @file Ensures `pnpm audit` only fails for advisories covered by the
 * frontend workspace ledger and a locally patched validator dependency.
 *
 * The validator package currently has no upstream patch release. We vendor the
 * required fix locally and treat the advisory as mitigated when the patched
 * code is present. Any additional vulnerabilities remain fatal.
 */
// biome-ignore assist/source/organizeImports: maintain external/node/local grouping required by review.
import { resolve } from 'node:path';
// biome-ignore assist/source/organizeImports: maintain external/node/local grouping required by review.
import { fileURLToPath } from 'node:url';

import auditExceptions from '../../security/audit-exceptions.json' with { type: 'json' };
import {
  collectAdvisories,
  partitionAdvisoriesById,
  reportUnexpectedAdvisories,
  runAuditJson,
} from '../../security/audit-utils.js';
import { VALIDATOR_ADVISORY_ID } from '../../security/constants.js';
import { isValidatorPatched } from '../../security/validator-patch.js';
import packageJson from '../package.json' with { type: 'json' };

const frontendPackageName = packageJson.name;
const workspaceKeys = new Set([
  frontendPackageName,
  frontendPackageName.includes('/')
    ? frontendPackageName.slice(frontendPackageName.lastIndexOf('/') + 1)
    : frontendPackageName,
]);
const frontendLedgerEntries = auditExceptions
  .filter((entry) => workspaceKeys.has(entry.package))
  .map((entry) => ({
    ...entry,
    expiryDate: entry.expiresAt ? new Date(entry.expiresAt) : null,
  }));
const frontendLedgerByAdvisoryId = new Map(
  frontendLedgerEntries.map((entry) => [entry.advisory, entry]),
);
const frontendAdvisoryIds = frontendLedgerEntries.map((entry) => entry.advisory);
const unexpectedHeading = 'Unexpected vulnerabilities detected by pnpm audit:';

/**
 * Determine whether the current module is executed as the entry script.
 *
 * @param {ImportMeta} meta Module metadata provided by Node.js.
 * @returns {boolean} Whether the script was launched directly via `node`.
 * @example
 * if (isExecutedDirectly(import.meta)) {
 *   console.log('Run from CLI');
 * }
 */
function isExecutedDirectly(meta) {
  const invokedPath = process.argv?.[1];
  if (!invokedPath) {
    return false;
  }

  try {
    const scriptPath = fileURLToPath(meta.url);
    const absoluteInvokedPath = resolve(invokedPath);
    return scriptPath === absoluteInvokedPath;
  } catch {
    return false;
  }
}

/**
 * Determine whether a ledger entry has a valid, non-expired expiry date.
 *
 * @param {{ expiryDate: Date | null, expiresAt?: string, id?: string, advisory: string }} entry
 *   Ledger entry augmented with a parsed expiry date.
 * @param {Date} [referenceDate=new Date()] Optional override for deterministic
 *   testing.
 * @returns {string | null} Error message describing the failure when the entry
 *   is missing an expiry date, has an invalid value, or has lapsed. Returns
 *   `null` when the entry remains valid.
 * @example
 * const error = getLedgerExpiryError({ expiryDate: new Date('2000-01-01'), expiresAt: '2000-01-01', advisory: 'GHSA-1' });
 * console.log(error);
 */
function getLedgerExpiryError(entry, referenceDate = new Date()) {
  if (!entry) {
    return null;
  }

  const entryLabel = entry.id ?? entry.advisory;

  if (!entry.expiresAt) {
    return `Audit exception ${entryLabel} for advisory ${entry.advisory} is missing an expiry date.`;
  }

  if (!(entry.expiryDate instanceof Date) || Number.isNaN(entry.expiryDate.valueOf())) {
    return `Audit exception ${entryLabel} for advisory ${entry.advisory} has an invalid expiry date (${entry.expiresAt}).`;
  }

  if (entry.expiryDate.getTime() < referenceDate.getTime()) {
    return `Audit exception ${entryLabel} for advisory ${entry.advisory} expired on ${entry.expiresAt}.`;
  }

  return null;
}

/**
 * Evaluate pnpm audit output and determine the appropriate exit code.
 *
 * @param {{ advisories?: Array<Record<string, unknown>>, status?: number }} payload Audit
 *   result containing advisories and the pnpm exit status.
 * @param {{ now?: Date }} [options] Optional evaluation parameters, primarily
 *   used by unit tests.
 * @returns {number} Exit code that should be returned to the shell.
 * @example
 * const exitCode = evaluateAudit({ advisories: [], status: 0 });
 * console.log(exitCode);
 */
export function evaluateAudit(payload, options = {}) {
  const { advisories: rawAdvisories = [], status } = payload;
  const statusCode = status ?? 0;
  const referenceDate = options.now ?? new Date();
  const { expected, unexpected } = partitionAdvisoriesById(rawAdvisories, frontendAdvisoryIds);

  const expiryErrors = expected
    .map((advisory) => frontendLedgerByAdvisoryId.get(advisory.github_advisory_id))
    .map((entry) => getLedgerExpiryError(entry, referenceDate))
    .filter((error) => Boolean(error));

  if (expiryErrors.length > 0) {
    for (const error of expiryErrors) {
      // biome-ignore lint/suspicious/noConsole: CLI script reports failures via stderr.
      console.error(error);
    }
    return 1;
  }

  if (reportUnexpectedAdvisories(unexpected, unexpectedHeading)) {
    return 1;
  }

  if (expected.length === 0) {
    return statusCode;
  }

  const hasValidatorAdvisory = expected.some(
    (advisory) => advisory.github_advisory_id === VALIDATOR_ADVISORY_ID,
  );

  if (hasValidatorAdvisory && !isValidatorPatched()) {
    // biome-ignore lint/suspicious/noConsole: CLI script reports failures via stderr.
    console.error(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} found but local patch missing.`,
    );
    return 1;
  }

  if (hasValidatorAdvisory) {
    const additionalCount = expected.length - 1;
    const suffix =
      additionalCount > 0
        ? ` (${additionalCount} additional ${additionalCount === 1 ? 'advisory' : 'advisories'} covered by ledger)`
        : '';
    // biome-ignore lint/suspicious/noConsole: CLI script reports status via stdout.
    console.info(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by local patch; audit passes.${suffix}`,
    );
  } else {
    // biome-ignore lint/suspicious/noConsole: CLI script reports status via stdout.
    console.info('All reported advisories are covered by the audit exception ledger.');
  }

  return 0;
}

/**
 * Execute `pnpm audit` and exit according to {@link evaluateAudit}.
 *
 * @returns {number} Exit code produced by {@link evaluateAudit}.
 * @example
 * const exitCode = main();
 * console.log(exitCode);
 */
export function main() {
  const { json, status } = runAuditJson();
  const advisories = collectAdvisories(json);
  return evaluateAudit({ advisories, status });
}

if (isExecutedDirectly(import.meta)) {
  try {
    const exitCode = main();
    process.exit(exitCode);
  } catch (error) {
    // biome-ignore lint/suspicious/noConsole: CLI script reports failures via stderr.
    console.error(error);
    process.exit(1);
  }
}
