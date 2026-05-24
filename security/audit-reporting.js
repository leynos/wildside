/** @file Reporting helpers for dependency audit advisory output. */

/** Return `true` when an advisory's GHSA ID is present in the allow-set.
 * @param {{ github_advisory_id?: string }} advisory Advisory to check. @param {Set<string>} allowed Set of permitted advisory IDs. @returns {boolean}
 * @example isExpectedAdvisory({ github_advisory_id: 'GHSA-vghf-hv5q-vc2g' }, new Set(['GHSA-vghf-hv5q-vc2g'])); // true
 */
function isExpectedAdvisory(advisory, allowed) {
  const id = advisory.github_advisory_id;
  return Boolean(id) && allowed.has(id);
}

/** Split advisories into allowed and unexpected groups.
 * @param {Array<{ github_advisory_id?: string }>} advisories Advisories to partition. @param {Iterable<string>} allowedIds Advisory IDs the caller expects.
 * @returns {{ expected: typeof advisories, unexpected: typeof advisories }} Partitioned advisories.
 * @example const { expected, unexpected } = partitionAdvisoriesById([{ github_advisory_id: 'GHSA-1' }, { github_advisory_id: 'GHSA-2' }], ['GHSA-2']); console.log(expected.length); // 1
 * @example console.log(unexpected.length); // 1
 */
export function partitionAdvisoriesById(advisories, allowedIds) {
  const allowed = new Set(allowedIds);
  const expected = [];
  const unexpected = [];
  for (const advisory of advisories) {
    if (isExpectedAdvisory(advisory, allowed)) {
      expected.push(advisory);
    } else {
      unexpected.push(advisory);
    }
  }

  return { expected, unexpected };
}

/** Format one advisory as a report line.
 * @param {{ github_advisory_id?: string, title?: string }} advisory Advisory to print. @returns {string} Human-readable bullet line for the advisory. @example formatAdvisoryLine({ github_advisory_id: 'GHSA-1', title: 'Example' }); // "- GHSA-1: Example"
 */
function formatAdvisoryLine(advisory) {
  const id = advisory.github_advisory_id ?? 'UNKNOWN';
  const suffix = advisory.title ? `: ${advisory.title}` : '';
  return `- ${id}${suffix}`;
}

/** Report unexpected advisories to stderr.
 * @param {Array<{ github_advisory_id?: string, title?: string }>} unexpected Advisories that were not permitted. @param {string} heading Descriptive heading for the error output.
 * @returns {boolean} Whether any advisories were reported. @example const hadUnexpected = reportUnexpectedAdvisories([{ github_advisory_id: 'GHSA-1', title: 'Example' }], 'Unexpected advisories:'); console.log(hadUnexpected); // true
 */
export function reportUnexpectedAdvisories(unexpected, heading) {
  if (unexpected.length === 0) {
    return false;
  }

  console.error(heading);
  for (const advisory of unexpected) {
    console.error(formatAdvisoryLine(advisory));
  }
  return true;
}
