/** @file Tests the Bun audit wrapper that consumes the exception ledger. */

import { describe, expect, it, vi } from 'vitest';
import { buildBunAuditArgs, runBunAudit } from '../security/run-bun-audit.js';

describe('buildBunAuditArgs', () => {
  it('deduplicates and sorts ledger advisory ignores', () => {
    expect(
      buildBunAuditArgs([
        { advisory: 'GHSA-vghf-hv5q-vc2g' },
        { advisory: 'GHSA-48c2-rrv3-qjmp' },
        { advisory: 'GHSA-vghf-hv5q-vc2g' },
      ]),
    ).toEqual([
      'audit',
      '--ignore=GHSA-48c2-rrv3-qjmp',
      '--ignore=GHSA-vghf-hv5q-vc2g',
    ]);
  });
});

describe('runBunAudit', () => {
  it('passes ledger advisories to bun audit as explicit ignores', () => {
    const spawnSync = vi.fn(() => ({ status: 0, signal: null }));

    expect(
      runBunAudit(
        [
          {
            addedAt: '2026-06-28',
            advisory: 'GHSA-vghf-hv5q-vc2g',
            expiresAt: '2026-07-28',
            id: 'TEST_EXCEPTION',
            package: 'validator',
            reason: 'Regression test fixture',
          },
        ],
        { spawnSync },
      ),
    ).toBe(0);

    expect(spawnSync).toHaveBeenCalledWith('bun', [
      'audit',
      '--ignore=GHSA-vghf-hv5q-vc2g',
    ], {
      stdio: 'inherit',
    });
  });

  it('throws when bun audit is signalled', () => {
    expect(() =>
      runBunAudit([], {
        spawnSync: () => ({ status: null, signal: 'SIGTERM' }),
      }),
    ).toThrow('bun audit was terminated by signal SIGTERM.');
  });

  it('returns the bun audit exit status when vulnerabilities are found', () => {
    expect(
      runBunAudit([], {
        spawnSync: () => ({ status: 1, signal: null }),
      }),
    ).toBe(1);
  });
});
