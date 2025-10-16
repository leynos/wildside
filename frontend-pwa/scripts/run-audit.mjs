/** @file Ensures `pnpm audit` only fails for known, patched validator vulnerability.
 *
 * The validator package currently has no upstream patch release. We vendor the
 * required fix locally and treat the advisory as mitigated when the patched
 * code is present. Any additional vulnerabilities remain fatal.
 */

import { spawnSync } from 'node:child_process';
import { isValidatorPatched } from '../../security/validator-patch.js';
const TARGET_ADVISORY = 'GHSA-9965-vmph-33xx';

function runAuditJson() {
  const result = spawnSync('pnpm', ['audit', '--json'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
  });

  if (result.error) {
    throw result.error;
  }

  if (!result.stdout) {
    throw new Error('pnpm audit produced no output to parse');
  }

  let parsed;
  try {
    parsed = JSON.parse(result.stdout);
  } catch (error) {
    error.message = `Failed to parse pnpm audit JSON: ${error.message}`;
    throw error;
  }

  return { parsed, status: result.status ?? 0 };
}

function main() {
  const { parsed, status } = runAuditJson();
  const advisories = Object.values(parsed.advisories ?? {});

  const unexpected = advisories.filter(
    (advisory) => advisory.github_advisory_id !== TARGET_ADVISORY,
  );

  if (unexpected.length > 0) {
    console.error('Unexpected vulnerabilities detected by pnpm audit:');
    for (const advisory of unexpected) {
      console.error(`- ${advisory.github_advisory_id}: ${advisory.title}`);
    }
    process.exit(1);
  }

  const targetFinding = advisories.find(
    (advisory) => advisory.github_advisory_id === TARGET_ADVISORY,
  );

  if (!targetFinding) {
    process.exit(status);
  }

  if (!isValidatorPatched()) {
    console.error(
      'Validator vulnerability GHSA-9965-vmph-33xx found but local patch missing.',
    );
    process.exit(1);
  }

  console.info(
    'Validator vulnerability GHSA-9965-vmph-33xx mitigated by local patch; audit passes.',
  );
  process.exit(0);
}

try {
  main();
} catch (error) {
  console.error(error);
  process.exit(1);
}
