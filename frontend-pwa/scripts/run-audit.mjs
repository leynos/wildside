/** @file Ensures the frontend audit only fails for advisories covered by the
 * workspace ledger and a validator dependency that includes the upstream fix
 * for the current advisory.
 *
 * The validator advisory is considered mitigated when the workspace ships a
 * version at or above the minimum safe release, falling back to the legacy
 * local patch marker for older builds. Any additional vulnerabilities remain
 * fatal.
 */
import { realpathSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import auditExceptions from '../../security/audit-exceptions.json' with { type: 'json' };
import {
  collectAdvisories,
  partitionAdvisoriesById,
  reportUnexpectedAdvisories,
  runAuditJson,
} from '../../security/audit-utils.js';
import { VALIDATOR_ADVISORY_ID, VALIDATOR_MIN_SAFE_VERSION } from '../../security/constants.js';
import { isValidatorPatched } from '../../security/validator-patch.js';
import packageJson from '../package.json' with { type: 'json' };

const normalize = (path) => realpathSync.native(path);

const frontendPackageName = packageJson.name;
const workspaceKeys = new Set([
  frontendPackageName,
  frontendPackageName.includes('/')
    ? frontendPackageName.slice(frontendPackageName.lastIndexOf('/') + 1)
    : frontendPackageName,
]);
const unexpectedHeading = 'Unexpected vulnerabilities detected by pnpm audit:';
// biome-ignore lint/style/useNamingConvention: constant emphasizes unit size.
const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * Index the ledger entries that apply to this workspace by advisory id.
 *
 * @param {Set<string>} workspaceKeys Workspace names the ledger may cover.
 * @param {Array<Record<string, unknown>>} auditEntries Ledger entries to filter.
 * @param {Date} referenceDate Date the ledger is evaluated against; validated.
 * @returns {{ ledgerByAdvisory: Map<string, Record<string, unknown>>, allowedIds: string[] }}
 *   Lookup by advisory id plus the covered ids in ledger order.
 */
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

/**
 * Describe why a ledger entry cannot cover an advisory.
 *
 * @returns {string | undefined} A failure message, or undefined when the entry
 *   is present, dated, and unexpired at the reference time.
 */
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
}

/**
 * Collect expiry failures for every advisory reported by the audit.
 *
 * @returns {string[]} One message per advisory whose ledger entry is unusable.
 */
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

/**
 * Report expiry failures to stderr.
 *
 * @returns {boolean} Whether any expiry failure was reported.
 */
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

/**
 * Report the validator advisory outcome and choose the exit code.
 *
 * @param {Array<Record<string, unknown>>} expected Advisories covered by the ledger.
 * @returns {number} Zero when the validator is patched or absent, otherwise one.
 */
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
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} found but validator < ${VALIDATOR_MIN_SAFE_VERSION} (or local patch missing).`,
    );
    return 1;
  }

  const additionalCount = expected.length - 1;
  const suffix =
    additionalCount > 0
      ? ` (${additionalCount} additional ${additionalCount === 1 ? 'advisory' : 'advisories'} covered by ledger)`
      : '';
  const mitigationPrefix = `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by validator >= ${VALIDATOR_MIN_SAFE_VERSION}; audit passes`;
  // biome-ignore lint/suspicious/noConsole: CLI script reports status via stdout.
  console.info(suffix ? `${mitigationPrefix}${suffix}` : mitigationPrefix);
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
  const invokedPath = process.argv[1];
  if (!invokedPath) {
    return false;
  }

  try {
    const scriptPath = fileURLToPath(meta.url);
    const absoluteInvokedPath = path.resolve(invokedPath);
    return normalize(scriptPath) === normalize(absoluteInvokedPath);
  } catch {
    return false;
  }
}

/**
 * Evaluate audit output and determine the appropriate exit code.
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
  const hasUnexpectedAdvisories = reportUnexpectedAdvisories(unexpected, unexpectedHeading);

  if (hasExpiredEntries || hasUnexpectedAdvisories) {
    return 1;
  }

  if (expected.length === 0) {
    return statusCode;
  }

  return reportValidatorOutcome(expected);
}

/**
 * Execute the audit helper and exit according to {@link evaluateAudit}.
 *
 * @returns {Promise<number>} Exit code produced by {@link evaluateAudit}.
 * @example
 * const exitCode = await main();
 * console.log(exitCode);
 */
export async function main() {
  const { json, status } = await runAuditJson();
  const advisories = collectAdvisories(json);
  return evaluateAudit({ advisories, status });
}

if (isExecutedDirectly(import.meta)) {
  try {
    const exitCode = await main();
    process.exitCode = exitCode;
  } catch (error) {
    // biome-ignore lint/suspicious/noConsole: CLI script reports failures via stderr.
    console.error(error);
    process.exitCode = 1;
  }
}
