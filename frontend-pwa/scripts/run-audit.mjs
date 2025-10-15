/** @file Ensures `pnpm audit` only fails for known, patched validator vulnerability.
 *
 * The validator package currently has no upstream patch release. We vendor the
 * required fix locally and treat the advisory as mitigated when the patched
 * code is present. Any additional vulnerabilities remain fatal.
 */

import { isValidatorPatched } from '../../security/validator-patch.js';
import {
  collectAdvisories,
  partitionAdvisoriesById,
  reportUnexpectedAdvisories,
  runAuditJson,
} from '../../security/audit-utils.js';
const TARGET_ADVISORY = 'GHSA-9965-vmph-33xx';

function main() {
  const { json, status } = runAuditJson();
  const advisories = collectAdvisories(json);
  const { expected, unexpected } = partitionAdvisoriesById(advisories, [TARGET_ADVISORY]);

  if (
    reportUnexpectedAdvisories(
      unexpected,
      'Unexpected vulnerabilities detected by pnpm audit:',
    )
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
