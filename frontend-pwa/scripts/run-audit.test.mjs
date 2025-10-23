/** @file Unit tests for the run-audit CLI helper. */

import { afterEach, describe, expect, it, vi } from 'vitest';

import { VALIDATOR_ADVISORY_ID } from '../../security/constants.js';

const validatorPatchMock = vi.fn(() => true);
const ledgerFixture = [
  {
    id: 'VAL-TEST-0001',
    package: 'frontend-pwa',
    advisory: VALIDATOR_ADVISORY_ID,
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

  if (patchApplied) {
    expect(validatorPatchMock).toHaveBeenCalledTimes(1);
  }

  expect(consoleSpy).toHaveBeenCalledWith(expectedMessage);
}

describe('evaluateAudit', () => {
  afterEach(() => {
    validatorPatchMock.mockReset();
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
    validatorPatchMock.mockReturnValue(true);
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const exitCode = evaluateAudit({
      advisories: [buildAdvisory('GHSA-0000-0000-0000', 'Unexpected')],
      status: 1,
    });

    expect(exitCode).toBe(1);
    expect(errorSpy).toHaveBeenCalledWith(
      expect.stringContaining('Unexpected vulnerabilities detected by pnpm audit:'),
    );
  });
});
