/** @file Unit and property tests for shared security audit helper modules. */

import fc from 'fast-check';
import { describe, expect, it, vi } from 'vitest';
import {
  buildVersionMap,
  collectInstalledPackageVersions,
  loadPackageTrees,
  normalizeBulkAdvisories,
  parseJsonOutput,
} from '../security/audit-package-data.js';
import { runAuditJson } from '../security/audit-utils.js';

function createCompletedResult(stdout = '[]') {
  return { error: undefined, signal: null, status: 0, stdout };
}

function assertCompletedProcess(result) {
  return result.status;
}

function mapToSortedObject(versionMap) {
  return Object.fromEntries(
    [...versionMap.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([packageName, versions]) => [packageName, [...versions].sort()]),
  );
}

describe('parseJsonOutput', () => {
  it('parses trimmed object and array payloads', () => {
    expect(parseJsonOutput('  {"advisories":{}}  ', 'pnpm audit')).toEqual({
      advisories: {},
    });
    expect(parseJsonOutput('\n[{"name":"frontend-pwa"}]\n', 'pnpm ls')).toEqual([
      { name: 'frontend-pwa' },
    ]);
  });

  it('returns an empty object for optional blank payloads', () => {
    expect(parseJsonOutput('   ', 'pnpm audit')).toEqual({});
    expect(parseJsonOutput(undefined, 'pnpm audit')).toEqual({});
  });

  it('throws labelled errors for required blank or malformed payloads', () => {
    expect(() =>
      parseJsonOutput('', 'bulk advisory audit', { requireNonEmpty: true }),
    ).toThrow('Failed to parse bulk advisory audit JSON: response body was empty.');
    expect(() => parseJsonOutput('{', 'pnpm audit')).toThrow(
      /^Failed to parse pnpm audit JSON:/,
    );
  });

  it.each([
    ['literal null', 'null', null],
    ['literal false', 'false', false],
    ['numeric zero', '0', 0],
  ])('preserves valid JSON edge case %#: %s', (_label, payload, expected) => {
    expect(parseJsonOutput(payload, 'pnpm audit')).toBe(expected);
  });
});

describe('buildVersionMap', () => {
  it('walks dependency sections recursively and skips workspace-local versions', () => {
    const versions = buildVersionMap([
      {
        dependencies: {
          '@app/types': { version: 'link:../packages/types' },
          validator: {
            version: '13.15.23',
            dependencies: {
              nanoid: { version: '3.3.11' },
            },
          },
        },
        devDependencies: {
          vitest: { version: '3.2.4' },
        },
        optionalDependencies: {
          fsevents: { version: '2.3.3' },
        },
      },
    ]);

    expect(mapToSortedObject(versions)).toEqual({
      fsevents: ['2.3.3'],
      nanoid: ['3.3.11'],
      validator: ['13.15.23'],
      vitest: ['3.2.4'],
    });
  });

  it('accepts a single null-prototype package tree', () => {
    const tree = Object.create(null);
    tree.dependencies = {
      validator: { version: '13.15.23' },
    };

    expect(mapToSortedObject(buildVersionMap(tree))).toEqual({
      validator: ['13.15.23'],
    });
  });

  it.each([null, new Map(), new Date()])(
    'rejects invalid dependency tree payload %#',
    (payload) => {
      expect(() => buildVersionMap(payload)).toThrow(
        'pnpm ls returned an invalid dependency tree payload.',
      );
    },
  );

  it('only records non-local string versions from generated dependency trees', () => {
    fc.assert(
      fc.property(
        fc.dictionary(
          fc.string({ minLength: 1 }).filter((name) => !name.includes('\0')),
          fc.oneof(
            fc.constantFrom('file:../pkg', 'link:../pkg', 'workspace:*', ''),
            fc.string({ minLength: 1 }).filter((version) =>
              !['file:', 'link:', 'workspace:'].some((prefix) => version.startsWith(prefix)),
            ),
          ),
          { maxKeys: 20 },
        ),
        (versionsByName) => {
          const tree = {
            dependencies: Object.fromEntries(
              Object.entries(versionsByName).map(([name, version]) => [
                name,
                { version },
              ]),
            ),
          };

          const actual = mapToSortedObject(buildVersionMap(tree));
          const expected = Object.fromEntries(
            Object.entries(versionsByName)
              .filter(([, version]) =>
                version &&
                !['file:', 'link:', 'workspace:'].some((prefix) => version.startsWith(prefix)),
              )
              .sort(([left], [right]) => left.localeCompare(right))
              .map(([name, version]) => [name, [version]]),
          );

          expect(actual).toEqual(expected);
        },
      ),
    );
  });
});

describe('loadPackageTrees and collectInstalledPackageVersions', () => {
  it('runs pnpm ls and returns parsed dependency trees', () => {
    const auditIo = {
      spawnSync: vi.fn(() =>
        createCompletedResult('[{"dependencies":{"validator":{"version":"13.15.23"}}}]'),
      ),
    };

    const trees = loadPackageTrees(auditIo, assertCompletedProcess);

    expect(trees).toEqual([
      { dependencies: { validator: { version: '13.15.23' } } },
    ]);
    expect(auditIo.spawnSync).toHaveBeenCalledWith(
      'pnpm',
      ['ls', '--json', '--depth', 'Infinity'],
      expect.objectContaining({ encoding: 'utf8' }),
    );
  });

  it('throws when pnpm ls exits non-zero or returns no tree', () => {
    expect(() =>
      loadPackageTrees(
        { spawnSync: () => createCompletedResult('[]') },
        () => 1,
      ),
    ).toThrow('pnpm ls failed without producing a dependency tree (exit status 1).');
    expect(() =>
      loadPackageTrees(
        { spawnSync: () => createCompletedResult('   ') },
        assertCompletedProcess,
      ),
    ).toThrow('pnpm ls failed without producing a dependency tree.');
  });

  it('serializes collected versions for the bulk advisory endpoint', () => {
    const auditIo = {
      spawnSync: () =>
        createCompletedResult(
          JSON.stringify([
            {
              dependencies: {
                validator: { version: '13.15.23' },
                nanoid: { version: '3.3.11' },
              },
            },
          ]),
        ),
    };

    expect(collectInstalledPackageVersions(auditIo, assertCompletedProcess)).toEqual({
      nanoid: ['3.3.11'],
      validator: ['13.15.23'],
    });
  });
});

describe('normalizeBulkAdvisories', () => {
  it('normalizes GHSA IDs, package names, and fallback keys', () => {
    expect(
      normalizeBulkAdvisories({
        validator: [
          {
            id: 100000,
            title: 'Validator SSRF',
            url: 'https://github.com/advisories/GHSA-Vghf-HV5Q-vC2G',
          },
          {
            id: 100001,
            title: 'Registry advisory',
            url: 'https://example.test/advisories/100001',
          },
        ],
      }),
    ).toEqual({
      'GHSA-vghf-hv5q-vc2g': {
        github_advisory_id: 'GHSA-vghf-hv5q-vc2g',
        id: 100000,
        package_name: 'validator',
        title: 'Validator SSRF',
        url: 'https://github.com/advisories/GHSA-Vghf-HV5Q-vC2G',
      },
      'validator:100001': {
        id: 100001,
        package_name: 'validator',
        title: 'Registry advisory',
        url: 'https://example.test/advisories/100001',
      },
    });
  });

  it('snapshots a normalized bulk advisory transformation', () => {
    expect(
      normalizeBulkAdvisories({
        ws: [
          {
            id: 110001,
            severity: 'moderate',
            title: 'Uninitialized memory disclosure',
            url: 'https://github.com/advisories/GHSA-58QX-3VCG-4XPX',
          },
        ],
      }),
    ).toMatchInlineSnapshot(`
      {
        "GHSA-58qx-3vcg-4xpx": {
          "github_advisory_id": "GHSA-58qx-3vcg-4xpx",
          "id": 110001,
          "package_name": "ws",
          "severity": "moderate",
          "title": "Uninitialized memory disclosure",
          "url": "https://github.com/advisories/GHSA-58QX-3VCG-4XPX",
        },
      }
    `);
  });

  it('rejects malformed bulk advisory payloads', () => {
    expect(() => normalizeBulkAdvisories(null)).toThrow(
      'Invalid bulk advisory payload: expected an object keyed by package name.',
    );
    expect(() => normalizeBulkAdvisories({ validator: {} })).toThrow(
      'Invalid bulk advisory entry for package validator: expected array',
    );
    expect(() => normalizeBulkAdvisories({ validator: [null] })).toThrow(
      'Invalid advisory for package validator at index 0: expected object',
    );
  });

  it('deduplicates generated advisories by canonical GHSA key', () => {
    fc.assert(
      fc.property(
        fc.array(
          fc.record({
            packageName: fc.string({ minLength: 1 }),
            id: fc.integer({ min: 1, max: 1_000_000 }),
            ghsa: fc.constantFrom(
              'GHSA-vghf-hv5q-vc2g',
              'GHSA-58qx-3vcg-4xpx',
              'GHSA-abcd-efgh-1234',
            ),
          }),
          { minLength: 1, maxLength: 30 },
        ),
        (entries) => {
          const payload = Object.create(null);
          for (const { packageName, id, ghsa } of entries) {
            payload[packageName] ??= [];
            payload[packageName].push({
              id,
              title: `Advisory ${id}`,
              url: `https://github.com/advisories/${ghsa.toUpperCase()}`,
            });
          }

          const normalized = normalizeBulkAdvisories(payload);
          const expectedKeys = new Set(
            entries.map(({ ghsa }) => `GHSA-${ghsa.slice('GHSA-'.length).toLowerCase()}`),
          );

          expect(new Set(Object.keys(normalized))).toEqual(expectedKeys);
          for (const key of expectedKeys) {
            expect(normalized[key].github_advisory_id).toBe(key);
          }
        },
      ),
    );
  });
});

describe('runAuditJson audit IO boundary', () => {
  it('reads npm registry settings through auditIo.getEnv before pnpm config', async () => {
    const auditIo = {
      clearTimeout: vi.fn(),
      execFileSync: vi.fn(() => {
        throw new Error('pnpm config should not be read');
      }),
      fetch: vi.fn(async () => ({
        ok: true,
        status: 200,
        statusText: 'OK',
        text: async () => '{}',
      })),
      getEnv: vi.fn((name) =>
        name === 'npm_config_registry' ? 'https://registry.example.test/custom' : undefined,
      ),
      setTimeout: vi.fn(() => 1),
      spawnSync: vi
        .fn()
        .mockReturnValueOnce(
          createCompletedResult(
            JSON.stringify({
              error: {
                code: 'ERR_PNPM_AUDIT_BAD_RESPONSE',
                message:
                  'The audit endpoint responded with 410: {"error":"This endpoint is being retired. Use the bulk advisory endpoint instead."}',
              },
            }),
          ),
        )
        .mockReturnValueOnce(createCompletedResult('[{"dependencies":{}}]')),
    };

    await expect(runAuditJson(auditIo)).resolves.toEqual({
      json: { advisories: {} },
      status: 0,
    });

    expect(auditIo.getEnv).toHaveBeenCalledWith('npm_config_registry');
    expect(auditIo.execFileSync).not.toHaveBeenCalled();
    expect(String(auditIo.fetch.mock.calls[0][0])).toBe(
      'https://registry.example.test/custom/-/npm/v1/security/advisories/bulk',
    );
  });
});
