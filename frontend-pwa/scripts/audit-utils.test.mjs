/** @file Tests the shared audit helper, including the bulk advisory fallback. */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const execFileSyncMock = vi.fn();
const spawnSyncMock = vi.fn();
const githubAdvisoryIdKey = 'github_advisory_id';
const packageNameKey = 'package_name';

vi.mock('node:child_process', () => ({
  execFileSync: execFileSyncMock,
  spawnSync: spawnSyncMock,
}));

const originalFetch = globalThis.fetch;

/**
 * Create a pnpm-like child-process result for audit command tests.
 * @param {{ status?: number, stdout?: string, error?: Error | undefined }} [options={}] Result overrides.
 * @param {number} [options.status=0] Process exit status.
 * @param {string} [options.stdout=''] Command stdout payload.
 * @param {Error | undefined} [options.error=undefined] Spawn error to surface.
 * @returns {{ error: Error | undefined, status: number, stdout: string }} Mocked pnpm result object.
 */
function createPnpmResult({ status = 0, stdout = '', error = undefined } = {}) {
  return { error, status, stdout };
}

/**
 * Configure the retired-endpoint pnpm audit flow for fallback tests.
 * @param {unknown[]} [lsPayload=[{ name: 'frontend-pwa', dependencies: {} }]] Parsed `pnpm ls` payload for the second mock result.
 * @returns {void}
 */
function setupRetiredPnpmAudit(lsPayload = [{ name: 'frontend-pwa', dependencies: {} }]) {
  spawnSyncMock
    .mockReturnValueOnce(
      createPnpmResult({
        status: 1,
        stdout: JSON.stringify({
          error: {
            code: 'ERR_PNPM_AUDIT_BAD_RESPONSE',
            message:
              'The audit endpoint responded with 410: {"error":"This endpoint is being retired. Use the bulk advisory endpoint instead."}',
          },
        }),
      }),
    )
    .mockReturnValueOnce(
      createPnpmResult({
        stdout: JSON.stringify(lsPayload),
      }),
    );
}

/**
 * Dynamically import the shared audit utility module under test.
 * @returns {Promise<typeof import('../../security/audit-utils.js')>} Imported audit utility module.
 */
async function loadAuditUtils() {
  const module = await import('../../security/audit-utils.js');
  return module;
}

describe('runAuditJson', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    globalThis.fetch = vi.fn();
    vi.unstubAllEnvs();
    vi.stubEnv('npm_config_registry', '');
    vi.stubEnv('NPM_CONFIG_REGISTRY', '');
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('returns pnpm audit output when the native command succeeds', async () => {
    spawnSyncMock.mockReturnValueOnce(
      createPnpmResult({
        status: 1,
        stdout: JSON.stringify({
          advisories: {
            validator: {
              [githubAdvisoryIdKey]: 'GHSA-vghf-hv5q-vc2g',
              title: 'Validator SSRF',
            },
          },
        }),
      }),
    );
    const { runAuditJson } = await loadAuditUtils();

    const result = await runAuditJson();

    expect(result).toEqual({
      json: {
        advisories: {
          validator: {
            [githubAdvisoryIdKey]: 'GHSA-vghf-hv5q-vc2g',
            title: 'Validator SSRF',
          },
        },
      },
      status: 1,
    });
    expect(fetch).not.toHaveBeenCalled();
    expect(execFileSyncMock).not.toHaveBeenCalled();
  });

  it('falls back to the bulk advisory endpoint when pnpm audit hits the retired endpoint', async () => {
    setupRetiredPnpmAudit([
      {
        name: 'frontend-pwa',
        path: '/tmp/frontend-pwa',
        dependencies: {
          '@app/types': {
            version: 'link:../packages/types',
          },
          validator: {
            version: '13.15.23',
            dependencies: {
              nanoid: {
                version: '3.3.11',
              },
            },
          },
        },
      },
    ]);
    execFileSyncMock.mockReturnValueOnce('https://registry.npmjs.org/\n');
    fetch.mockResolvedValueOnce({
      ok: true,
      status: 200,
      statusText: 'OK',
      text: async () =>
        JSON.stringify({
          validator: [
            {
              id: 100000,
              url: 'https://github.com/advisories/GHSA-vghf-hv5q-vc2g',
              title: 'Validator SSRF',
              severity: 'high',
            },
          ],
          nanoid: [],
        }),
    });
    const { runAuditJson } = await loadAuditUtils();

    const result = await runAuditJson();

    expect(spawnSyncMock).toHaveBeenNthCalledWith(
      1,
      'pnpm',
      ['audit', '--json'],
      expect.objectContaining({ encoding: 'utf8' }),
    );
    expect(spawnSyncMock).toHaveBeenNthCalledWith(
      2,
      'pnpm',
      ['ls', '--json', '--depth', 'Infinity'],
      expect.objectContaining({ encoding: 'utf8' }),
    );
    expect(execFileSyncMock).toHaveBeenCalledWith(
      'pnpm',
      ['config', 'get', 'registry'],
      expect.objectContaining({ encoding: 'utf8' }),
    );
    expect(String(fetch.mock.calls[0][0])).toBe(
      'https://registry.npmjs.org/-/npm/v1/security/advisories/bulk',
    );
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual({
      nanoid: ['3.3.11'],
      validator: ['13.15.23'],
    });
    expect(result).toEqual({
      json: {
        advisories: {
          'GHSA-vghf-hv5q-vc2g': {
            [githubAdvisoryIdKey]: 'GHSA-vghf-hv5q-vc2g',
            id: 100000,
            [packageNameKey]: 'validator',
            severity: 'high',
            title: 'Validator SSRF',
            url: 'https://github.com/advisories/GHSA-vghf-hv5q-vc2g',
          },
        },
      },
      status: 1,
    });
  });

  it('throws a clear error when the bulk advisory endpoint fails', async () => {
    setupRetiredPnpmAudit();
    fetch.mockResolvedValueOnce({
      ok: false,
      status: 503,
      statusText: 'Service Unavailable',
      text: async () => '{"error":"upstream unavailable"}',
    });
    const { runAuditJson } = await loadAuditUtils();

    await expect(runAuditJson()).rejects.toThrow(
      'Bulk advisory audit failed (503 Service Unavailable)',
    );
  });

  it('preserves advisory ID casing from the bulk payload URL', async () => {
    setupRetiredPnpmAudit();
    fetch.mockResolvedValueOnce({
      ok: true,
      status: 200,
      statusText: 'OK',
      text: async () =>
        JSON.stringify({
          validator: [
            {
              id: 100000,
              url: 'https://github.com/advisories/GHSA-Vghf-HV5Q-vC2G',
              title: 'Validator SSRF',
            },
          ],
        }),
    });
    const { runAuditJson } = await loadAuditUtils();

    const result = await runAuditJson();

    expect(result.json.advisories).toEqual({
      'GHSA-Vghf-HV5Q-vC2G': expect.objectContaining({
        [githubAdvisoryIdKey]: 'GHSA-Vghf-HV5Q-vC2G',
        [packageNameKey]: 'validator',
      }),
    });
  });

  it('rejects blank bulk advisory responses instead of treating them as empty JSON', async () => {
    setupRetiredPnpmAudit();
    fetch.mockResolvedValueOnce({
      ok: true,
      status: 200,
      statusText: 'OK',
      text: async () => '   ',
    });
    const { runAuditJson } = await loadAuditUtils();

    await expect(runAuditJson()).rejects.toThrow(
      'Failed to parse bulk advisory audit JSON: response body was empty.',
    );
  });
});
