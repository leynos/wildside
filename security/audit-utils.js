/** @file Shared helpers for running `pnpm audit` and reasoning about advisories.
 *
 * These helpers centralise the JSON parsing and filtering logic used by the
 * security validation scripts. They ensure both the security gate and
 * workspace-specific audit wrappers interpret the CLI output consistently.
 *
 * Cross-link: `frontend-pwa/scripts/run-audit.mjs` consumes these helpers to
 * enforce the validator patch requirement during workspace audits.
 */

import { spawnSync } from 'node:child_process';

/**
 * Run `pnpm audit --json` and return the parsed payload alongside the exit
 * status. Whitespace-only output is treated as an empty advisory list so that
 * callers can rely on deterministic results even when pnpm prints nothing.
 *
 * @returns {{ json: Record<string, unknown>, status: number }} Parsed audit
 *   output and the pnpm exit status (defaults to zero when undefined).
 * @example
 * const { json, status } = runAuditJson();
 * if (status !== 0) {
 *   throw new Error('pnpm audit failed');
 * }
 * console.log(Object.keys(json.advisories ?? {}));
 */
export function runAuditJson() {
  const result = spawnSync('pnpm', ['audit', '--json'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
  });

  if (result.error) {
    throw result.error;
  }

  const status = result.status ?? 0;
  const stdout = result.stdout ? result.stdout.trim() : '';

  if (!stdout) {
    return { json: { advisories: {} }, status };
  }

  try {
    return { json: JSON.parse(stdout), status };
  } catch (error) {
    error.message = `Failed to parse pnpm audit JSON: ${error.message}`;
    throw error;
  }
}

/**
 * Convert the advisories object returned by `pnpm audit` into a flat array that
 * is easier to filter.
 *
 * @param {{ advisories?: Record<string, unknown> }} auditJson Raw JSON payload
 *   from `pnpm audit`.
 * @returns {Array<Record<string, unknown>>} List of advisory objects.
 * @example
 * const advisories = collectAdvisories({ advisories: { "GHSA-123": { id: 1 } } });
 * console.log(advisories.length); // 1
 */
export function collectAdvisories(auditJson) {
  return Object.values(auditJson.advisories ?? {});
}

/**
 * Split advisories into those whose GitHub advisory IDs are present in the
 * allowed list and those that are unexpected.
 *
 * @param {Array<{ github_advisory_id?: string }>} advisories Advisories to
 *   partition.
 * @param {Iterable<string>} allowedIds Advisory IDs the caller expects.
 * @returns {{ expected: typeof advisories, unexpected: typeof advisories }}
 *   Partitioned advisories.
 * @example
 * const { expected, unexpected } = partitionAdvisoriesById(
 *   [
 *     { github_advisory_id: 'GHSA-1' },
 *     { github_advisory_id: 'GHSA-2' },
 *   ],
 *   ['GHSA-2'],
 * );
 * console.log(expected.length); // 1
 * console.log(unexpected.length); // 1
 */
export function partitionAdvisoriesById(advisories, allowedIds) {
  const allowed = new Set(allowedIds);
  const expected = [];
  const unexpected = [];

  for (const advisory of advisories) {
    const id = advisory.github_advisory_id;
    if (id && allowed.has(id)) {
      expected.push(advisory);
    } else {
      unexpected.push(advisory);
    }
  }

  return { expected, unexpected };
}

/**
 * Report unexpected advisories to stderr.
 *
 * @param {Array<{ github_advisory_id?: string, title?: string }>} unexpected
 *   Advisories that were not permitted.
 * @param {string} heading Descriptive heading for the error output.
 * @returns {boolean} Whether any advisories were reported.
 * @example
 * const hadUnexpected = reportUnexpectedAdvisories(
 *   [{ github_advisory_id: 'GHSA-1', title: 'Example' }],
 *   'Unexpected advisories:',
 * );
 * console.log(hadUnexpected); // true
 */
export function reportUnexpectedAdvisories(unexpected, heading) {
  if (unexpected.length === 0) {
    return false;
  }

  console.error(heading);
  for (const advisory of unexpected) {
    const id = advisory.github_advisory_id ?? 'UNKNOWN';
    const suffix = advisory.title ? `: ${advisory.title}` : '';
    console.error(`- ${id}${suffix}`);
  }
  return true;
}
