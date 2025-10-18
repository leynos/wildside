/** @file Ensures `pnpm audit` only fails for known, patched validator vulnerability.
 *
 * The validator package currently has no upstream patch release. We vendor the
 * required fix locally and treat the advisory as mitigated when the patched
 * code is present. Any additional vulnerabilities remain fatal.
 */
// biome-ignore assist/source/organizeImports: maintain external/node/local grouping required by review.
import { VALIDATOR_ADVISORY_ID } from '../../security/constants.js';
import { isValidatorPatched } from '../../security/validator-patch.js';
import {
  collectAdvisories,
  partitionAdvisoriesById,
  reportUnexpectedAdvisories,
  runAuditJson,
} from '../../security/audit-utils.js';
// biome-ignore lint/style/useNamingConvention: advisory identifier mirrors upstream notation.
const TARGET_ADVISORY = VALIDATOR_ADVISORY_ID;

function main() {
  const { json, status } = runAuditJson();
  const advisories = collectAdvisories(json);
  const { expected, unexpected } = partitionAdvisoriesById(advisories, [TARGET_ADVISORY]);

  if (
    reportUnexpectedAdvisories(unexpected, 'Unexpected vulnerabilities detected by pnpm audit:')
  ) {
    process.exit(1);
  }

  const targetFinding = expected.find(
    (advisory) => advisory.github_advisory_id === TARGET_ADVISORY,
  );

  if (!targetFinding) {
    process.exit(status);
  }

  if (!isValidatorPatched()) {
    // biome-ignore lint/suspicious/noConsole: CLI script reports failures via stderr.
    console.error('Validator vulnerability GHSA-9965-vmph-33xx found but local patch missing.');
    process.exit(1);
  }

  // biome-ignore lint/suspicious/noConsole: CLI script reports status via stdout.
  console.info(
    'Validator vulnerability GHSA-9965-vmph-33xx mitigated by local patch; audit passes.',
  );
  process.exit(0);
}

try {
  main();
} catch (error) {
  // biome-ignore lint/suspicious/noConsole: CLI script reports failures via stderr.
  console.error(error);
  process.exit(1);
}
