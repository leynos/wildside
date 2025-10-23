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
  frontendPackageName.includes('/') ? frontendPackageName.split('/').pop() : frontendPackageName,
]);
const UNEXPECTED_HEADING = 'Unexpected vulnerabilities detected by pnpm audit:';

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
 * Ensure the audit exception entry remains valid.
 *
 * @param {{ id?: string, advisory?: string, package?: string, expiresAt?: string }} entry
 *   Ledger entry describing an allowed advisory.
 * @param {Date} [now=new Date()] Timestamp used to evaluate expiry.
 * @throws {Error} When the entry has expired or the expiry timestamp is invalid.
 * @example
 * assertExceptionActive({ advisory: 'GHSA-1', expiresAt: '2099-01-01' });
 */
function assertExceptionActive(entry, now = new Date()) {
  if (!entry.expiresAt) {
    return;
  }

  const expiry = new Date(entry.expiresAt);
  if (Number.isNaN(expiry.getTime())) {
    throw new Error(
      `Ledger exception ${entry.id ?? entry.advisory ?? entry.package} has invalid expiry`,
    );
  }

  if (expiry < now) {
    throw new Error(
      `Ledger exception ${entry.id ?? entry.advisory ?? entry.package} expired on ${entry.expiresAt}`,
    );
  }
}

/**
 * Load advisory IDs that this workspace may treat as mitigated.
 *
 * @param {Date} [now=new Date()] Timestamp used to validate ledger expiry.
 * @returns {string[]} Advisory identifiers permitted for this workspace.
 * @example
 * const allowed = loadWorkspaceAdvisoryIds(new Date('2025-02-15'));
 * console.log(allowed.includes('GHSA-9965-vmph-33xx'));
 */
function loadWorkspaceAdvisoryIds(now = new Date()) {
  const ids = [];

  for (const entry of auditExceptions) {
    if (!workspaceKeys.has(entry.package)) {
      continue;
    }

    assertExceptionActive(entry, now);
    ids.push(entry.advisory);
  }

  return ids;
}

/**
 * Evaluate pnpm audit output and determine the appropriate exit code.
 *
 * @param {{ advisories: Array<Record<string, unknown>>, status: number }} payload Audit
 *   result containing advisories and the pnpm exit status.
 * @param {{ now?: Date }} [options] Optional evaluation parameters.
 * @returns {number} Exit code that should be returned to the shell.
 * @example
 * const exitCode = evaluateAudit({ advisories: [], status: 0 });
 * console.log(exitCode);
 */
export function evaluateAudit(payload, options = {}) {
  const now = options.now ?? new Date();
  const allowedIds = loadWorkspaceAdvisoryIds(now);
  const advisories = payload.advisories ?? [];
  const { expected, unexpected } = partitionAdvisoriesById(advisories, allowedIds);

  if (reportUnexpectedAdvisories(unexpected, UNEXPECTED_HEADING)) {
    return 1;
  }

  if (expected.length === 0) {
    return payload.status;
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
    // biome-ignore lint/suspicious/noConsole: CLI script reports status via stdout.
    console.info(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by local patch; audit passes.`,
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
