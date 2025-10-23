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

describe('evaluateAudit', () => {
  afterEach(() => {
    validatorPatchMock.mockReset();
    vi.restoreAllMocks();
  });

  it('passes when the validator advisory is mitigated locally', async () => {
    const { evaluateAudit } = await import('./run-audit.mjs');
    validatorPatchMock.mockReturnValue(true);
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});

    const exitCode = evaluateAudit({
      advisories: [buildAdvisory(VALIDATOR_ADVISORY_ID)],
      status: 1,
    });

    expect(exitCode).toBe(0);
    expect(validatorPatchMock).toHaveBeenCalledTimes(1);
    expect(infoSpy).toHaveBeenCalledWith(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by local patch; audit passes.`,
    );
  });

  it('fails when the validator advisory is present but the patch is missing', async () => {
    const { evaluateAudit } = await import('./run-audit.mjs');
    validatorPatchMock.mockReturnValue(false);
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const exitCode = evaluateAudit({
      advisories: [buildAdvisory(VALIDATOR_ADVISORY_ID)],
      status: 1,
    });

    expect(exitCode).toBe(1);
    expect(errorSpy).toHaveBeenCalledWith(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} found but local patch missing.`,
    );
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
