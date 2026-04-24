/** @file Tests the override parity helper and guarded CLI entrypoint. */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fileURLToPath } from 'node:url';

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
const SYNCED = { 'basic-ftp': '5.3.0', dompurify: '3.4.0', uuid: '14.0.0' };

/**
 * Load a fresh module and immediately invoke checkOverridesParity.
 *
 * @param {object} packageJson - The package.json fixture to check.
 * @returns {Promise<number>} The exit-code returned by checkOverridesParity.
 */
async function runParityCheck(packageJson) {
  const { checkOverridesParity } = await loadModule();
  return checkOverridesParity(packageJson);
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
  let consoleLogSpy;
  let consoleErrorSpy;

  beforeEach(() => {
    vi.clearAllMocks();
    consoleLogSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleLogSpy.mockRestore();
    consoleErrorSpy.mockRestore();
  });

  it('returns 0 and logs success when both override blocks match', async () => {
    const result = await runParityCheck({
      overrides: SYNCED,
      pnpm: { overrides: SYNCED },
    });

    expect(result).toBe(0);
    expect(consoleLogSpy).toHaveBeenCalledWith(
      expect.stringContaining('basic-ftp, dompurify, uuid'),
    );
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('returns 1 and reports the mismatched dependency version', async () => {
    const result = await runParityCheck({
      overrides: {
        'basic-ftp': '5.3.0',
        dompurify: '3.3.0',
        uuid: '14.0.0',
      },
      pnpm: { overrides: SYNCED },
    });

    expect(result).toBe(1);
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      expect.stringContaining('Override mismatch for "dompurify":'),
    );
    expect(consoleLogSpy).not.toHaveBeenCalled();
  });

  it('returns 1 when the top-level overrides block is absent', async () => {
    const result = await runParityCheck({ pnpm: { overrides: SYNCED } });

    expect(result).toBe(1);
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      expect.stringContaining('overrides: <missing>'),
    );
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      expect.stringContaining('pnpm.overrides: "5.3.0"'),
    );
  });

  it('returns 1 when the pnpm overrides block is absent', async () => {
    const result = await runParityCheck({ overrides: SYNCED });

    expect(result).toBe(1);
    expect(consoleErrorSpy).toHaveBeenCalled();
    expect(consoleLogSpy).not.toHaveBeenCalled();
  });

  it('returns 1 when an individual top-level entry is missing', async () => {
    const result = await runParityCheck({
      overrides: {
        dompurify: '3.4.0',
        uuid: '14.0.0',
      },
      pnpm: { overrides: SYNCED },
    });

    expect(result).toBe(1);
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      expect.stringContaining('Override mismatch for "basic-ftp":'),
    );
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      expect.stringContaining('overrides: <missing>'),
    );
  });

  it('returns 1 when both override blocks are absent', async () => {
    const result = await runParityCheck({});

    expect(result).toBe(1);
    expect(consoleErrorSpy).toHaveBeenCalled();
    expect(consoleLogSpy).not.toHaveBeenCalled();
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
          'basic-ftp': '5.3.0',
          dompurify: '3.4.0',
          uuid: '14.0.0',
        },
        pnpm: {
          overrides: {
            'basic-ftp': '5.3.0',
            dompurify: '3.4.0',
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
      'Override parity verified for basic-ftp, dompurify, uuid.',
    );
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
