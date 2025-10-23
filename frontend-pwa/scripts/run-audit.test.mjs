/** @file Exercises the audit wrapper to ensure ledger exceptions behave as expected. */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { VALIDATOR_ADVISORY_ID } from '../../security/constants.js';

const validatorPatchMock = vi.fn(() => true);

const baselineLedgerEntries = [
  {
    id: 'VAL-2025-0001',
    package: 'frontend-pwa',
    advisory: VALIDATOR_ADVISORY_ID,
    reason: 'Local patch hardens isURL protocol parsing until upstream ships a fix.',
    addedAt: '2025-02-14',
    expiresAt: '2026-02-14',
  },
  {
    id: 'VAL-2025-0002',
    package: 'backend-service',
    advisory: 'GHSA-aaaa-bbbb-cccc',
    reason: 'Backend-only exception ensures unrelated workspaces fail fast.',
    addedAt: '2025-02-14',
    expiresAt: '2026-02-14',
  },
];

function cloneLedgerEntries() {
  return baselineLedgerEntries.map((entry) => ({ ...entry }));
}

const ledgerEntries = cloneLedgerEntries();

function resetLedgerEntries() {
  ledgerEntries.splice(0, ledgerEntries.length, ...cloneLedgerEntries());
}

vi.mock('../../security/audit-exceptions.json', () => ({
  default: ledgerEntries,
}));

vi.mock('../../security/validator-patch.js', () => ({
  isValidatorPatched: validatorPatchMock,
}));

const createAdvisory = (id, title) => ({
  // biome-ignore lint/style/useNamingConvention: matches pnpm audit JSON keys.
  github_advisory_id: id,
  title,
});

async function loadEvaluateAudit() {
  const { evaluateAudit } = await import('./run-audit.mjs');
  return evaluateAudit;
}

const evaluateAuditScenarios = [
  {
    name: 'returns success when advisories are covered by the ledger',
    advisories: [createAdvisory(VALIDATOR_ADVISORY_ID, 'validator vulnerability')],
    spyFactory: () => vi.spyOn(console, 'info').mockImplementation(() => {}),
    expectedExitCode: 0,
    expectedValidatorCalls: 1,
    assertSpy: (spy) => {
      expect(spy).toHaveBeenCalledWith(
        `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by local patch; audit passes.`,
      );
    },
  },
  {
    name: 'propagates failure when unexpected advisories are reported',
    advisories: [createAdvisory('GHSA-abcd-1234-efgh', 'Unexpected vulnerability')],
    spyFactory: () => vi.spyOn(console, 'error').mockImplementation(() => {}),
    expectedExitCode: 1,
    expectedValidatorCalls: 0,
    assertSpy: (spy) => {
      expect(spy).toHaveBeenCalledWith(
        expect.stringContaining('Unexpected vulnerabilities detected by pnpm audit:'),
      );
    },
  },
];

describe('evaluateAudit', () => {
  beforeEach(() => {
    resetLedgerEntries();
    vi.resetModules();
    validatorPatchMock.mockReset();
    validatorPatchMock.mockReturnValue(true);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it.each(evaluateAuditScenarios)(
    '$name',
    async ({ advisories, spyFactory, expectedExitCode, expectedValidatorCalls, assertSpy }) => {
      const evaluateAudit = await loadEvaluateAudit();
      const consoleSpy = spyFactory();

      const exitCode = evaluateAudit({ advisories, status: 1 });

      expect(exitCode).toBe(expectedExitCode);
      if (typeof expectedValidatorCalls === 'number') {
        expect(validatorPatchMock).toHaveBeenCalledTimes(expectedValidatorCalls);
      }
      assertSpy(consoleSpy);
    },
  );

  it('fails when validator advisory is present but local patch is missing', async () => {
    const evaluateAudit = await loadEvaluateAudit();
    validatorPatchMock.mockReturnValue(false);
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const exitCode = evaluateAudit({
      advisories: [createAdvisory(VALIDATOR_ADVISORY_ID, 'validator vulnerability')],
      status: 1,
    });

    expect(exitCode).toBe(1);
    expect(validatorPatchMock).toHaveBeenCalledTimes(1);
    expect(errorSpy).toHaveBeenCalledWith(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} found but local patch missing.`,
    );
  });

  it('notes additional ledger-covered advisories when validator patch applies', async () => {
    ledgerEntries.push({
      id: 'VAL-2025-0003',
      package: 'frontend-pwa',
      advisory: 'GHSA-wxyz-9876-hijk',
      reason: 'Secondary advisory accepted temporarily for the frontend.',
      addedAt: '2025-02-14',
      expiresAt: '2026-02-14',
    });
    const evaluateAudit = await loadEvaluateAudit();
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});

    const exitCode = evaluateAudit({
      advisories: [
        createAdvisory(VALIDATOR_ADVISORY_ID, 'validator vulnerability'),
        createAdvisory('GHSA-wxyz-9876-hijk', 'secondary issue'),
      ],
      status: 1,
    });

    expect(exitCode).toBe(0);
    expect(validatorPatchMock).toHaveBeenCalledTimes(1);
    expect(infoSpy).toHaveBeenCalledWith(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by local patch; audit passes. (1 additional advisory covered by ledger)`,
    );
  });

  const ledgerExpiryErrorScenarios = [
    {
      name: 'fails when a ledger exception has expired',
      setupAction: () => {
        ledgerEntries[0].expiresAt = '2024-02-14';
      },
      expectedErrorMessage: `Audit exception VAL-2025-0001 for advisory ${VALIDATOR_ADVISORY_ID} expired on 2024-02-14.`,
    },
    {
      name: 'fails when a ledger exception is missing an expiry date',
      setupAction: () => {
        delete ledgerEntries[0].expiresAt;
      },
      expectedErrorMessage: `Audit exception VAL-2025-0001 for advisory ${VALIDATOR_ADVISORY_ID} is missing an expiry date.`,
    },
  ];

  it.each(ledgerExpiryErrorScenarios)('$name', async ({ setupAction, expectedErrorMessage }) => {
    setupAction();
    const evaluateAudit = await loadEvaluateAudit();
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const exitCode = evaluateAudit({
      advisories: [createAdvisory(VALIDATOR_ADVISORY_ID, 'validator vulnerability')],
      status: 1,
    });

    expect(exitCode).toBe(1);
    expect(errorSpy).toHaveBeenCalledWith(expectedErrorMessage);
  });

  it('reports coverage when advisories are covered solely by the ledger', async () => {
    ledgerEntries.push({
      id: 'VAL-2025-0004',
      package: 'frontend-pwa',
      advisory: 'GHSA-ledg-erpk-1000',
      reason: 'Non-validator advisory permitted temporarily for the frontend.',
      addedAt: '2025-02-14',
      expiresAt: '2026-02-14',
    });
    const evaluateAudit = await loadEvaluateAudit();
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});

    const exitCode = evaluateAudit({
      advisories: [createAdvisory('GHSA-ledg-erpk-1000', 'example permitted advisory')],
      status: 1,
    });

    expect(exitCode).toBe(0);
    expect(infoSpy).toHaveBeenCalledWith(
      'All reported advisories are covered by the audit exception ledger.',
    );
    expect(validatorPatchMock).not.toHaveBeenCalled();
  });
});
