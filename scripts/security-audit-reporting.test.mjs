/** @file Unit and property tests for audit reporting and exception policy. */

import fc from 'fast-check';
import { describe, expect, it, vi } from 'vitest';
import { assertNoExpired } from '../security/audit-exception-policy.js';
import {
  partitionAdvisoriesById as partitionAdvisoriesByIdFromUtils,
  reportUnexpectedAdvisories as reportUnexpectedAdvisoriesFromUtils,
} from '../security/audit-utils.js';
import {
  partitionAdvisoriesById,
  reportUnexpectedAdvisories,
} from '../security/audit-reporting.js';

function advisory(id, title = `Advisory ${id}`) {
  return { github_advisory_id: id, title };
}

function exceptionEntry({ addedAt, expiresAt, id = 'exception-1' }) {
  return {
    addedAt,
    advisory: 'GHSA-vghf-hv5q-vc2g',
    expiresAt,
    id,
    package: 'validator',
    reason: 'Regression test fixture',
  };
}

function throwingPolicyIo() {
  return {
    error: vi.fn(),
    exit: vi.fn((code) => {
      throw new Error(`exit ${code}`);
    }),
  };
}

describe('partitionAdvisoriesById', () => {
  it('is re-exported from the shared audit utility surface', () => {
    expect(partitionAdvisoriesByIdFromUtils).toBe(partitionAdvisoriesById);
  });

  it('partitions advisories without reordering either group', () => {
    const first = advisory('GHSA-1111-2222-3333');
    const second = advisory('GHSA-4444-5555-6666');
    const third = { title: 'Missing GHSA' };

    expect(partitionAdvisoriesById([first, second, third], [second.github_advisory_id])).toEqual({
      expected: [second],
      unexpected: [first, third],
    });
  });

  it('keeps every generated advisory in exactly one partition', () => {
    fc.assert(
      fc.property(
        fc.uniqueArray(fc.uuid(), { minLength: 1, maxLength: 30 }),
        fc.array(fc.boolean(), { minLength: 1, maxLength: 30 }),
        (ids, flags) => {
          const advisories = ids.map((id) => advisory(id));
          const allowedIds = ids.filter((_, index) => flags[index % flags.length]);
          const { expected, unexpected } = partitionAdvisoriesById(advisories, allowedIds);

          expect(expected).toHaveLength(new Set(allowedIds).size);
          expect([...expected, ...unexpected].sort((left, right) =>
            left.github_advisory_id.localeCompare(right.github_advisory_id),
          )).toEqual([...advisories].sort((left, right) =>
            left.github_advisory_id.localeCompare(right.github_advisory_id),
          ));
        },
      ),
    );
  });
});

describe('reportUnexpectedAdvisories', () => {
  it('is re-exported from the shared audit utility surface', () => {
    expect(reportUnexpectedAdvisoriesFromUtils).toBe(reportUnexpectedAdvisories);
  });

  it('returns false and writes nothing for an empty report', () => {
    const reportingIo = { error: vi.fn() };

    expect(reportUnexpectedAdvisories([], 'Unexpected advisories:', reportingIo)).toBe(false);
    expect(reportingIo.error).not.toHaveBeenCalled();
  });

  it('reports unexpected advisories to the injected reportingIo adapter', () => {
    const errorLines = [];
    const reportingIo = { error: (...args) => errorLines.push(args.join(' ')) };

    expect(
      reportUnexpectedAdvisories(
        [advisory('GHSA-1', 'Example')],
        'Unexpected advisories:',
        reportingIo,
      ),
    ).toBe(true);

    expect(errorLines).toMatchInlineSnapshot(`
      [
        "Unexpected advisories:",
        "- GHSA-1: Example",
      ]
    `);
  });
});

describe('assertNoExpired', () => {
  it('uses the injected current date instead of wall-clock time', () => {
    const policyIo = throwingPolicyIo();

    assertNoExpired(
      [exceptionEntry({ addedAt: '2024-01-01', expiresAt: '2024-01-31' })],
      new Date('2024-01-30T00:00:00.000Z'),
      policyIo,
    );

    expect(policyIo.exit).not.toHaveBeenCalled();
    expect(policyIo.error).not.toHaveBeenCalled();
  });

  it('allows exceptions expiring on or after the current date', () => {
    fc.assert(
      fc.property(fc.date({ min: new Date('2024-01-01'), max: new Date('2030-12-31') }), (date) => {
        const today = date.toISOString().slice(0, 10);
        const policyIo = throwingPolicyIo();

        assertNoExpired(
          [
            exceptionEntry({
              addedAt: '2024-01-01',
              expiresAt: today,
            }),
          ],
          date,
          policyIo,
        );

        expect(policyIo.exit).not.toHaveBeenCalled();
      }),
    );
  });

  it.each([
    [
      'expires before the current date',
      exceptionEntry({ addedAt: '2024-01-01', expiresAt: '2024-01-31' }),
      new Date('2024-02-01T00:00:00.000Z'),
      [
        'Audit exceptions have expired:',
        '- exception-1 (validator) expired on 2024-01-31',
      ],
    ],
    [
      'has an inverted date range',
      exceptionEntry({ addedAt: '2024-02-01', expiresAt: '2024-01-31' }),
      new Date('2024-01-15T00:00:00.000Z'),
      [
        'Audit exceptions have invalid date ranges (addedAt > expiresAt):',
        '- exception-1 (validator) addedAt 2024-02-01 > expiresAt 2024-01-31',
      ],
    ],
  ])('exits when an exception %s', (_description, entry, currentDate, expectedErrors) => {
    const policyIo = throwingPolicyIo();

    expect(() => assertNoExpired([entry], currentDate, policyIo)).toThrow('exit 1');
    expect(policyIo.error.mock.calls.map(([line]) => line)).toEqual(expectedErrors);
  });
});
