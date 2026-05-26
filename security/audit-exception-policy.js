/**
 * @file Audit exception ledger policy: validates time-bound dependency-audit
 * exception entries and enforces expiry at validation time.
 *
 * Owns the `assertNoExpired` export, which reads an array of exception entries
 * (each carrying `id`, `package`, `addedAt`, and `expiresAt` ISO-date fields),
 * computes today's date relative to an injected `currentDate`, and exits with
 * status 1 when any entry has expired or carries an invalid date range
 * (`addedAt` later than `expiresAt`).
 *
 * Relationships:
 * - `security/validate-audit.js` imports `assertNoExpired` and invokes it after
 *   loading `security/audit-exceptions.json` through AJV schema validation.
 * - Test suites (`scripts/security-audit-reporting.test.mjs`) import
 *   `assertNoExpired` directly with injected `policyIo` adapters to exercise
 *   expiry and inverted-range paths without calling `process.exit`.
 *
 * IO effects are isolated behind the `policyIo` adapter (`error`, `exit`); the
 * `defaultPolicyIo` implementation delegates to `console.error` and
 * `process.exit`.
 */

const defaultPolicyIo = {
  error: (...args) => console.error(...args),
  exit: (code) => process.exit(code),
};

/** Exit with error if any audit exceptions are past their expiry date.
 * @param {Array<{ id: string, package: string, addedAt: string, expiresAt: string }>} entries Entries to inspect.
 * @param {Date} [currentDate=new Date()] Current date for deterministic validation.
 * @param {{ error: (...args: unknown[]) => void, exit: (code: number) => never }} [policyIo=defaultPolicyIo] Policy IO adapter.
 * @returns {void}
 * @example assertNoExpired([{ id: '1', package: 'pkg', addedAt: '2024-01-01', expiresAt: '2099-01-01' }]);
 */
export function assertNoExpired(entries, currentDate = new Date(), policyIo = defaultPolicyIo) {
  const today = currentDate.toISOString().slice(0, 10);
  const expired = entries.filter((e) => e.expiresAt < today);
  const inverted = entries.filter((e) => e.addedAt > e.expiresAt);
  if (expired.length > 0) {
    policyIo.error('Audit exceptions have expired:');
    for (const { id, package: pkg, expiresAt } of expired) {
      policyIo.error(`- ${id} (${pkg}) expired on ${expiresAt}`);
    }
    policyIo.exit(1);
  }
  if (inverted.length > 0) {
    policyIo.error('Audit exceptions have invalid date ranges (addedAt > expiresAt):');
    for (const { id, package: pkg, addedAt, expiresAt } of inverted) {
      policyIo.error(`- ${id} (${pkg}) addedAt ${addedAt} > expiresAt ${expiresAt}`);
    }
    policyIo.exit(1);
  }
}
