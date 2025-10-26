/** @file Ensures `pnpm audit` only fails for advisories covered by the
 * frontend workspace ledger and a locally patched validator dependency.
 *
 * The validator package currently has no upstream patch release. We vendor the
 * required fix locally and treat the advisory as mitigated when the patched
 * code is present. Any additional vulnerabilities remain fatal.
 */
import { realpathSync } from 'node:fs';
import { resolve } from 'node:path';
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
const unexpectedHeading = 'Unexpected vulnerabilities detected by pnpm audit:';
// biome-ignore lint/style/useNamingConvention: constant emphasises unit size.
const DAY_MS = 24 * 60 * 60 * 1000;

function buildLedgerMaps(workspaceKeys, auditEntries, referenceDate) {
  if (!(referenceDate instanceof Date) || Number.isNaN(referenceDate.getTime())) {
    throw new TypeError('Invalid reference date');
  }

  const ledgerByAdvisory = new Map();
  const allowedIds = [];

  for (const entry of auditEntries) {
    if (!workspaceKeys.has(entry.package)) {
      continue;
    }

    ledgerByAdvisory.set(entry.advisory, entry);
    allowedIds.push(entry.advisory);
  }

  return { ledgerByAdvisory, allowedIds };
}

function getLedgerExpiryError(entry, advisoryId, referenceDateValue) {
  if (!entry) {
    return `Audit ledger entry missing for advisory ${advisoryId ?? 'unknown'}.`;
  }

  const entryLabel = entry.id ?? entry.advisory;

  if (!entry.expiresAt) {
    return `Audit exception ${entryLabel} for advisory ${entry.advisory} is missing an expiry date.`;
  }

  const rawExpiry = String(entry.expiresAt).trim();
  const expiryDate = new Date(rawExpiry);

  if (Number.isNaN(expiryDate.valueOf())) {
    return `Audit exception ${entryLabel} for advisory ${entry.advisory} has an invalid expiry date (raw: ${rawExpiry || '<empty>'}, expected ISO 8601).`;
  }

  const dateOnlyPattern = /^\d{4}-\d{2}-\d{2}$/;
  const expiryBoundary = dateOnlyPattern.test(rawExpiry)
    ? expiryDate.getTime() + DAY_MS
    : expiryDate.getTime();

  if (expiryBoundary <= referenceDateValue) {
    return `Audit exception ${entryLabel} for advisory ${entry.advisory} expired on ${rawExpiry}.`;
  }

  return null;
}

function collectAdvisoryExpiryErrors(advisories, ledgerByAdvisory, referenceDateValue) {
  const errors = [];

  for (const advisory of advisories) {
    const entry = ledgerByAdvisory.get(advisory.github_advisory_id ?? '');
    const error = getLedgerExpiryError(entry, advisory.github_advisory_id, referenceDateValue);
    if (error) {
      errors.push(error);
    }
  }

  return errors;
}

function reportExpiryFailures(expected, ledgerByAdvisory, referenceDateValue) {
  const expiryErrors = collectAdvisoryExpiryErrors(expected, ledgerByAdvisory, referenceDateValue);

  if (expiryErrors.length === 0) {
    return false;
  }

  for (const error of expiryErrors) {
    // biome-ignore lint/suspicious/noConsole: CLI script reports failures via stderr.
    console.error(error);
  }

  return true;
}

function reportUnexpectedFailures(unexpected) {
  return reportUnexpectedAdvisories(unexpected, unexpectedHeading);
}

function reportValidatorOutcome(expected) {
  const sawValidator = expected.some(
    (advisory) => advisory.github_advisory_id === VALIDATOR_ADVISORY_ID,
  );

  if (!sawValidator) {
    // biome-ignore lint/suspicious/noConsole: CLI script reports status via stdout.
    console.info('All reported advisories are covered by the audit exception ledger.');
    return 0;
  }

  if (!isValidatorPatched()) {
    // biome-ignore lint/suspicious/noConsole: CLI script reports failures via stderr.
    console.error(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} found but local patch missing.`,
    );
    return 1;
  }

  const additionalCount = expected.length - 1;
  const suffix =
    additionalCount > 0
      ? ` (${additionalCount} additional ${additionalCount === 1 ? 'advisory' : 'advisories'} covered by ledger)`
      : '';
  // biome-ignore lint/suspicious/noConsole: CLI script reports status via stdout.
  console.info(
    `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by local patch; audit passes.${suffix}`,
  );
  return 0;
}

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
    const normalise = (path) =>
      typeof realpathSync.native === 'function' ? realpathSync.native(path) : realpathSync(path);
    return normalise(scriptPath) === normalise(absoluteInvokedPath);
  } catch {
    return false;
  }
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
  const referenceDate = options.now ?? new Date();
  const referenceDateValue = referenceDate.getTime();
  const rawAdvisories = payload.advisories ?? [];
  const statusCode = payload.status ?? 0;

  const { ledgerByAdvisory, allowedIds } = buildLedgerMaps(
    workspaceKeys,
    auditExceptions,
    referenceDate,
  );

  const { expected, unexpected } = partitionAdvisoriesById(rawAdvisories, allowedIds);

  const hasExpiredEntries = reportExpiryFailures(expected, ledgerByAdvisory, referenceDateValue);
  const hasUnexpectedAdvisories = reportUnexpectedFailures(unexpected);

  if (hasExpiredEntries || hasUnexpectedAdvisories) {
    return 1;
  }

  if (expected.length === 0) {
    return statusCode;
  }

  return reportValidatorOutcome(expected);
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
