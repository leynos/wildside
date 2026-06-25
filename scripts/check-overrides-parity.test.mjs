/** @file Tests the override parity helper and guarded CLI entrypoint. */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import packageJson from '../package.json' with { type: 'json' };

const readFileMock = vi.fn();

vi.mock('node:fs/promises', () => ({
  readFile: readFileMock,
}));

const moduleUrl = new URL('./check-overrides-parity.mjs', import.meta.url);
const modulePath = fileURLToPath(moduleUrl);

/**
 * Load a fresh copy of the module under test after resetting the module cache.
 *
 * @returns {Promise<typeof import('./check-overrides-parity.mjs')>} The imported module.
 */
async function loadModule() {
  vi.resetModules();
  return import('./check-overrides-parity.mjs');
}

/** Convenience fixture data shared across parity tests. */
const DOMPURIFY_OVERRIDE = packageJson.overrides.dompurify;
assert.equal(
  packageJson.pnpm.overrides.dompurify,
  DOMPURIFY_OVERRIDE,
  'package.json pnpm.overrides.dompurify must match overrides.dompurify',
);
assert.equal(
  typeof DOMPURIFY_OVERRIDE,
  'string',
  'package.json overrides.dompurify must be a string',
);
const DRIFTED_DOMPURIFY_OVERRIDE = `not-${DOMPURIFY_OVERRIDE}`;

const SYNCED = {
  'basic-ftp': '5.3.1',
  dompurify: DOMPURIFY_OVERRIDE,
  'ip-address': '10.1.1',
  uuid: '14.0.0',
};

/**
 * Load a fresh module and immediately invoke checkOverridesParity.
 *
 * @param {object} packageJson - The package.json fixture to check.
 * @returns {Promise<ReturnType<typeof import('./check-overrides-parity.mjs').checkOverridesParity>>} The parity report.
 */
async function runParityCheck(packageJson) {
  const { checkOverridesParity } = await loadModule();
  return checkOverridesParity(packageJson);
}

/**
 * Build expected mismatch rows when one override block is missing entirely.
 *
 * @param {'root' | 'pnpm'} missingSide - The override side expected to be absent.
 * @returns {Array<{dependencyName: string, rootValue: string | undefined, pnpmValue: string | undefined}>} Expected mismatch rows.
 */
function expectedMissingOverrideMismatches(missingSide) {
  return Object.entries(SYNCED).map(([dependencyName, overrideValue]) => ({
    dependencyName,
    rootValue: missingSide === 'root' ? undefined : overrideValue,
    pnpmValue: missingSide === 'pnpm' ? undefined : overrideValue,
  }));
}

describe('formatOverrideValue', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns <missing> when the value is undefined', async () => {
    const { formatOverrideValue } = await loadModule();

    expect(formatOverrideValue(undefined)).toBe('<missing>');
  });

  it('stringifies string values for diagnostics', async () => {
    const { formatOverrideValue } = await loadModule();

    expect(formatOverrideValue('5.3.0')).toBe('"5.3.0"');
  });
});

describe('checkOverridesParity', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns a successful report when both override blocks match', async () => {
    const result = await runParityCheck({
      overrides: SYNCED,
      pnpm: { overrides: SYNCED },
    });

    expect(result).toEqual({
      ok: true,
      overridesToCheck: ['basic-ftp', 'dompurify', 'ip-address', 'uuid'],
      mismatches: [],
      reason: 'matched',
    });
  });

  it('returns structured mismatches for drifted dependency versions', async () => {
    const result = await runParityCheck({
      overrides: {
        'basic-ftp': '5.3.1',
        dompurify: DRIFTED_DOMPURIFY_OVERRIDE,
        'ip-address': '10.1.1',
        uuid: '14.0.0',
      },
      pnpm: { overrides: SYNCED },
    });

    expect(result).toMatchObject({
      ok: false,
      reason: 'mismatched',
      mismatches: [
        {
          dependencyName: 'dompurify',
          rootValue: DRIFTED_DOMPURIFY_OVERRIDE,
          pnpmValue: DOMPURIFY_OVERRIDE,
        },
      ],
    });
  });

  it('reports each missing top-level override entry', async () => {
    const result = await runParityCheck({ pnpm: { overrides: SYNCED } });

    expect(result.ok).toBe(false);
    expect(result.mismatches).toEqual(expectedMissingOverrideMismatches('root'));
  });

  it('reports each missing pnpm override entry', async () => {
    const result = await runParityCheck({ overrides: SYNCED });

    expect(result.ok).toBe(false);
    expect(result.mismatches).toEqual(expectedMissingOverrideMismatches('pnpm'));
  });

  it('reports an individual missing top-level entry', async () => {
    const result = await runParityCheck({
      overrides: {
        dompurify: DOMPURIFY_OVERRIDE,
        'ip-address': '10.1.1',
        uuid: '14.0.0',
      },
      pnpm: { overrides: SYNCED },
    });

    expect(result.mismatches).toEqual([
      { dependencyName: 'basic-ftp', rootValue: undefined, pnpmValue: '5.3.1' },
    ]);
  });

  it('reports an individual missing pnpm entry when shared values match', async () => {
    const result = await runParityCheck({
      overrides: SYNCED,
      pnpm: {
        overrides: {
          dompurify: DOMPURIFY_OVERRIDE,
          'ip-address': '10.1.1',
          uuid: '14.0.0',
        },
      },
    });

    expect(result.overridesToCheck).toEqual(['basic-ftp', 'dompurify', 'ip-address', 'uuid']);
    expect(result.mismatches).toEqual([
      { dependencyName: 'basic-ftp', rootValue: '5.3.1', pnpmValue: undefined },
    ]);
  });

  it('reports missing overrides when both override blocks are absent', async () => {
    const result = await runParityCheck({});

    expect(result).toEqual({
      ok: false,
      overridesToCheck: [],
      mismatches: [],
      reason: 'missing-overrides',
    });
  });
});

describe('reportOverridesParity', () => {
  it('logs success and returns 0 for a passing report', async () => {
    const { reportOverridesParity } = await loadModule();
    const outputIo = { log: vi.fn(), error: vi.fn() };

    const exitCode = reportOverridesParity(
      {
        ok: true,
        overridesToCheck: ['basic-ftp', 'dompurify'],
        mismatches: [],
        reason: 'matched',
      },
      outputIo,
    );

    expect(exitCode).toBe(0);
    expect(outputIo.log).toHaveBeenCalledWith(
      'Override parity verified for basic-ftp, dompurify.',
    );
    expect(outputIo.error).not.toHaveBeenCalled();
  });

  it('logs mismatches and returns 1 for a failing report', async () => {
    const { reportOverridesParity } = await loadModule();
    const outputIo = { log: vi.fn(), error: vi.fn() };

    const exitCode = reportOverridesParity(
      {
        ok: false,
        overridesToCheck: ['dompurify'],
        mismatches: [
          {
            dependencyName: 'dompurify',
            rootValue: DRIFTED_DOMPURIFY_OVERRIDE,
            pnpmValue: DOMPURIFY_OVERRIDE,
          },
        ],
        reason: 'mismatched',
      },
      outputIo,
    );

    expect(exitCode).toBe(1);
    expect(outputIo.error).toHaveBeenCalledWith(
      [
        'Override parity check failed.',
        'Override mismatch for "dompurify":',
        `  overrides: "${DRIFTED_DOMPURIFY_OVERRIDE}"`,
        `  pnpm.overrides: "${DOMPURIFY_OVERRIDE}"`,
      ].join('\n'),
    );
    expect(outputIo.log).not.toHaveBeenCalled();
  });
});

describe('direct execution guard', () => {
  let originalArgv;
  let originalExitCode;
  let consoleLogSpy;
  let consoleErrorSpy;

  beforeEach(() => {
    vi.clearAllMocks();
    originalArgv = [...process.argv];
    originalExitCode = process.exitCode;
    process.exitCode = undefined;
    consoleLogSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    process.argv = originalArgv;
    process.exitCode = originalExitCode;
    consoleLogSpy.mockRestore();
    consoleErrorSpy.mockRestore();
  });

  it('reads package.json and sets exitCode when invoked directly', async () => {
    readFileMock.mockResolvedValueOnce(
      JSON.stringify({
        overrides: {
          'basic-ftp': '5.3.1',
          dompurify: DOMPURIFY_OVERRIDE,
          'ip-address': '10.1.1',
          uuid: '14.0.0',
        },
        pnpm: {
          overrides: {
            'basic-ftp': '5.3.1',
            dompurify: DOMPURIFY_OVERRIDE,
            'ip-address': '10.1.1',
            uuid: '14.0.0',
          },
        },
      }),
    );
    process.argv = [process.argv[0], modulePath];

    await loadModule();

    expect(readFileMock).toHaveBeenCalledTimes(1);
    expect(process.exitCode).toBe(0);
    expect(consoleLogSpy).toHaveBeenCalledWith(
      'Override parity verified for basic-ftp, dompurify, ip-address, uuid.',
    );
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
