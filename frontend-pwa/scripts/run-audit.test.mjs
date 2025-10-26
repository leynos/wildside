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

const cloneLedgerEntries = (entries = baselineLedgerEntries) =>
  entries.map((entry) => ({ ...entry }));

let nextLedgerEntries = cloneLedgerEntries();

function setLedgerEntries(mutator) {
  const snapshot = cloneLedgerEntries();
  if (typeof mutator === 'function') {
    const result = mutator(snapshot);
    nextLedgerEntries = Array.isArray(result) ? result : snapshot;
    return;
  }

  nextLedgerEntries = snapshot;
}

vi.mock('../../security/audit-exceptions.json', () => ({
  get default() {
    return cloneLedgerEntries(nextLedgerEntries);
  },
}));

vi.mock('../../security/validator-patch.js', () => ({
  isValidatorPatched: validatorPatchMock,
}));

const createAdvisory = (id, title) => ({
  // biome-ignore lint/style/useNamingConvention: matches pnpm audit JSON keys.
  github_advisory_id: id,
  title,
});

// biome-ignore lint/style/useNamingConvention: constant date shared across cases.
const DEFAULT_NOW = new Date('2025-03-01T00:00:00.000Z');

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
    setLedgerEntries();
    vi.resetModules();
    vi.clearAllMocks();
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

      const exitCode = evaluateAudit({ advisories, status: 1 }, { now: DEFAULT_NOW });

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

    const exitCode = evaluateAudit(
      {
        advisories: [createAdvisory(VALIDATOR_ADVISORY_ID, 'validator vulnerability')],
        status: 1,
      },
      { now: DEFAULT_NOW },
    );

    expect(exitCode).toBe(1);
    expect(validatorPatchMock).toHaveBeenCalledTimes(1);
    expect(errorSpy).toHaveBeenCalledWith(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} found but local patch missing.`,
    );
  });

  it('notes additional ledger-covered advisories when validator patch applies', async () => {
    setLedgerEntries((entries) => {
      entries.push({
        id: 'VAL-2025-0003',
        package: 'frontend-pwa',
        advisory: 'GHSA-wxyz-9876-hijk',
        reason: 'Secondary advisory accepted temporarily for the frontend.',
        addedAt: '2025-02-14',
        expiresAt: '2026-02-14',
      });
      return entries;
    });
    const evaluateAudit = await loadEvaluateAudit();
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});

    const exitCode = evaluateAudit(
      {
        advisories: [
          createAdvisory(VALIDATOR_ADVISORY_ID, 'validator vulnerability'),
          createAdvisory('GHSA-wxyz-9876-hijk', 'secondary issue'),
        ],
        status: 1,
      },
      { now: DEFAULT_NOW },
    );

    expect(exitCode).toBe(0);
    expect(validatorPatchMock).toHaveBeenCalledTimes(1);
    expect(infoSpy).toHaveBeenCalledWith(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by local patch; audit passes. (1 additional advisory covered by ledger)`,
    );
  });

  it('notes plural additional ledger-covered advisories when validator patch applies', async () => {
    setLedgerEntries((entries) => {
      entries.push(
        {
          id: 'VAL-2025-0003',
          package: 'frontend-pwa',
          advisory: 'GHSA-wxyz-9876-hijk',
          reason: 'Secondary advisory accepted temporarily for the frontend.',
          addedAt: '2025-02-14',
          expiresAt: '2026-02-14',
        },
        {
          id: 'VAL-2025-0004',
          package: 'frontend-pwa',
          advisory: 'GHSA-lmno-5432-pqrs',
          reason: 'Tertiary advisory accepted temporarily for the frontend.',
          addedAt: '2025-02-14',
          expiresAt: '2026-02-14',
        },
      );
      return entries;
    });
    const evaluateAudit = await loadEvaluateAudit();
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});

    const exitCode = evaluateAudit(
      {
        advisories: [
          createAdvisory(VALIDATOR_ADVISORY_ID, 'validator vulnerability'),
          createAdvisory('GHSA-wxyz-9876-hijk', 'secondary issue'),
          createAdvisory('GHSA-lmno-5432-pqrs', 'tertiary issue'),
        ],
        status: 1,
      },
      { now: DEFAULT_NOW },
    );

    expect(exitCode).toBe(0);
    expect(validatorPatchMock).toHaveBeenCalledTimes(1);
    expect(infoSpy).toHaveBeenCalledWith(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by local patch; audit passes. (2 additional advisories covered by ledger)`,
    );
  });

  it('returns success when only ledger-covered non-validator advisories are reported', async () => {
    setLedgerEntries((entries) => {
      entries.push({
        id: 'VAL-2025-0003',
        package: 'frontend-pwa',
        advisory: 'GHSA-wxyz-9876-hijk',
        reason: 'Non-validator advisory accepted temporarily for the frontend.',
        addedAt: '2025-02-14',
        expiresAt: '2026-02-14',
      });
      return entries;
    });
    const evaluateAudit = await loadEvaluateAudit();
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const exitCode = evaluateAudit(
      {
        advisories: [createAdvisory('GHSA-wxyz-9876-hijk', 'secondary issue')],
        status: 1,
      },
      { now: DEFAULT_NOW },
    );

    expect(exitCode).toBe(0);
    expect(validatorPatchMock).not.toHaveBeenCalled();
    expect(infoSpy).toHaveBeenCalledWith(
      'All reported advisories are covered by the audit exception ledger.',
    );
    expect(errorSpy).not.toHaveBeenCalled();
  });

  const ledgerExpiryErrorScenarios = [
    {
      name: 'fails when a ledger exception has expired',
      setupAction: () => {
        setLedgerEntries((entries) => {
          entries[0].expiresAt = '2024-02-14';
          return entries;
        });
      },
      expectedErrorMessage: `Audit exception VAL-2025-0001 for advisory ${VALIDATOR_ADVISORY_ID} expired on 2024-02-14.`,
    },
    {
      name: 'fails when a ledger exception is missing an expiry date',
      setupAction: () => {
        setLedgerEntries((entries) => {
          delete entries[0].expiresAt;
          return entries;
        });
      },
      expectedErrorMessage: `Audit exception VAL-2025-0001 for advisory ${VALIDATOR_ADVISORY_ID} is missing an expiry date.`,
    },
    {
      name: 'fails once a date-only expiry boundary passes',
      setupAction: () => {
        setLedgerEntries((entries) => {
          entries[0].expiresAt = '2025-02-14';
          return entries;
        });
      },
      expectedErrorMessage: `Audit exception VAL-2025-0001 for advisory ${VALIDATOR_ADVISORY_ID} expired on 2025-02-14.`,
      referenceDate: new Date('2025-02-15T00:00:00.001Z'),
    },
    {
      name: 'fails when a ledger exception has an invalid expiry date',
      setupAction: () => {
        setLedgerEntries((entries) => {
          entries[0].expiresAt = 'not-a-date';
          return entries;
        });
      },
      expectedErrorMessage: `Audit exception VAL-2025-0001 for advisory ${VALIDATOR_ADVISORY_ID} has an invalid expiry date (raw: not-a-date, expected ISO 8601).`,
    },
  ];

  it.each(ledgerExpiryErrorScenarios)(
    '$name',
    async ({ setupAction, expectedErrorMessage, referenceDate }) => {
      setupAction();
      const evaluateAudit = await loadEvaluateAudit();
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      const exitCode = evaluateAudit(
        {
          advisories: [createAdvisory(VALIDATOR_ADVISORY_ID, 'validator vulnerability')],
          status: 1,
        },
        { now: referenceDate ?? DEFAULT_NOW },
      );

      expect(exitCode).toBe(1);
      expect(errorSpy).toHaveBeenCalledWith(expectedErrorMessage);
    },
  );

  it('allows date-only expiry on the same day', async () => {
    setLedgerEntries((entries) => {
      entries[0].expiresAt = '2025-02-14';
      return entries;
    });
    const evaluateAudit = await loadEvaluateAudit();
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});

    const exitCode = evaluateAudit(
      {
        advisories: [createAdvisory(VALIDATOR_ADVISORY_ID, 'validator vulnerability')],
        status: 1,
      },
      { now: new Date('2025-02-14T23:59:59.999Z') },
    );

    expect(exitCode).toBe(0);
    expect(infoSpy).toHaveBeenCalledWith(
      `Validator vulnerability ${VALIDATOR_ADVISORY_ID} mitigated by local patch; audit passes.`,
    );
  });

  it('logs both expiry and unexpected advisory failures in one run', async () => {
    setLedgerEntries((entries) => {
      entries[0].expiresAt = '2024-02-14';
      return entries;
    });
    const evaluateAudit = await loadEvaluateAudit();
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const exitCode = evaluateAudit(
      {
        advisories: [
          createAdvisory(VALIDATOR_ADVISORY_ID, 'validator vulnerability'),
          createAdvisory('GHSA-unexp-0000-0000', 'unexpected advisory'),
        ],
        status: 1,
      },
      { now: DEFAULT_NOW },
    );

    expect(exitCode).toBe(1);
    expect(errorSpy).toHaveBeenCalledWith(
      `Audit exception VAL-2025-0001 for advisory ${VALIDATOR_ADVISORY_ID} expired on 2024-02-14.`,
    );
    expect(errorSpy).toHaveBeenCalledWith(
      expect.stringContaining('Unexpected vulnerabilities detected by pnpm audit:'),
    );
  });

  it('passes through status when no advisories are present', async () => {
    const evaluateAudit = await loadEvaluateAudit();
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const successCode = evaluateAudit({ advisories: [], status: 0 }, { now: DEFAULT_NOW });
    const failureCode = evaluateAudit({ advisories: [], status: 1 }, { now: DEFAULT_NOW });

    expect(successCode).toBe(0);
    expect(failureCode).toBe(1);
    expect(infoSpy).not.toHaveBeenCalled();
    expect(errorSpy).not.toHaveBeenCalled();
  });
});
