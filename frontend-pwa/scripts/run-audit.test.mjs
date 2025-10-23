/** @file Unit tests for the run-audit CLI helper. */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { VALIDATOR_ADVISORY_ID } from '../../security/constants.js';

const validatorPatchMock = vi.fn(() => true);
const ledgerFixture = [
  {
    id: 'VAL-TEST-0001',
    package: 'frontend-pwa',
    advisory: VALIDATOR_ADVISORY_ID,
    reason: 'Local patch hardens validator URL handling until upstream ships a fix.',
    addedAt: '2025-02-14',
    expiresAt: '2099-01-01',
  },
  {
    id: 'VAL-TEST-0002',
    package: 'frontend-pwa',
    advisory: 'GHSA-ledg-erpk-1000',
    reason: 'Example ledger entry ensuring non-validator advisories pass cleanly.',
    addedAt: '2025-02-14',
    expiresAt: '2099-01-01',
  },
  {
    id: 'VAL-TEST-0003',
    package: 'backend-service',
    advisory: 'GHSA-aaaa-bbbb-cccc',
    reason: 'Backend exception verifies workspace-level filtering.',
    addedAt: '2025-02-14',
    expiresAt: '2099-01-01',
  },
];

vi.mock('../../security/validator-patch.js', () => ({
  isValidatorPatched: validatorPatchMock,
}));
vi.mock('../../security/audit-exceptions.json', () => ({
  default: ledgerFixture,
}));

/**
 * Build a pnpm advisory object matching the audit schema.
 *
 * @param {string} id GitHub advisory identifier.
 * @param {string} [title] Human-readable title when present.
 * @returns {Record<string, unknown>} Advisory payload for tests.
 * @example
 * const advisory = buildAdvisory('GHSA-1', 'Example');
 * console.log(advisory.github_advisory_id);
 */
function buildAdvisory(id, title) {
  return {
    // biome-ignore lint/style/useNamingConvention: matches pnpm audit schema.
    github_advisory_id: id,
    title,
  };
}

/**
 * Run evaluateAudit against the validator advisory with configurable outcome expectations.
 *
 * @param {{ patchApplied: boolean, expectedExitCode: number, consoleMethod: 'info' | 'error', expectedMessage: string }} options
 *   Parameters describing the desired test scenario.
 * @returns {Promise<void>} Resolves once evaluateAudit assertions complete.
 */
async function testValidatorAdvisory({
  patchApplied,
  expectedExitCode,
  consoleMethod,
  expectedMessage,
}) {
  const { evaluateAudit } = await import('./run-audit.mjs');
  validatorPatchMock.mockReturnValue(patchApplied);
  const consoleSpy = vi.spyOn(console, consoleMethod).mockImplementation(() => {});

  const exitCode = evaluateAudit({
    advisories: [buildAdvisory(VALIDATOR_ADVISORY_ID)],
    status: 1,
  });

  expect(exitCode).toBe(expectedExitCode);
  expect(validatorPatchMock).toHaveBeenCalledTimes(1);
  expect(consoleSpy).toHaveBeenCalledWith(expectedMessage);
}

describe('evaluateAudit', () => {
  beforeEach(() => {
    vi.resetModules();
    validatorPatchMock.mockReset();
    validatorPatchMock.mockReturnValue(true);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('passes when the validator advisory is mitigated locally', async () => {
    await testValidatorAdvisory({
      patchApplied: true,
      expectedExitCode: 0,
      consoleMethod: 'info',
      expectedMessage: `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by local patch; audit passes.`,
    });
  });

  it('fails when the validator advisory is present but the patch is missing', async () => {
    await testValidatorAdvisory({
      patchApplied: false,
      expectedExitCode: 1,
      consoleMethod: 'error',
      expectedMessage: `Validator vulnerability ${VALIDATOR_ADVISORY_ID} found but local patch missing.`,
    });
  });

  it('fails when unexpected advisories are reported', async () => {
    const { evaluateAudit } = await import('./run-audit.mjs');
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const exitCode = evaluateAudit({
      advisories: [buildAdvisory('GHSA-0000-0000-0000', 'Unexpected')],
      status: 1,
    });

    expect(exitCode).toBe(1);
    expect(errorSpy).toHaveBeenCalled();
    expect(errorSpy.mock.calls[0][0]).toBe('Unexpected vulnerabilities detected by pnpm audit:');
  });

  it('reports coverage when advisories are covered by the ledger', async () => {
    const { evaluateAudit } = await import('./run-audit.mjs');
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});

    const exitCode = evaluateAudit({
      advisories: [buildAdvisory('GHSA-ledg-erpk-1000', 'Example permitted advisory')],
      status: 1,
    });

    expect(exitCode).toBe(0);
    expect(infoSpy).toHaveBeenCalledWith(
      'All reported advisories are covered by the audit exception ledger.',
    );
    expect(validatorPatchMock).not.toHaveBeenCalled();
  });
});
