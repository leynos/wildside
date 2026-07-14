/** @file Tests the override policy helper and guarded CLI entrypoint. */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fileURLToPath } from 'node:url';
import packageJson from '../package.json' with { type: 'json' };

const readFileMock = vi.fn();

vi.mock('node:fs/promises', () => ({
  readFile: readFileMock,
}));

const moduleUrl = new URL('./check-overrides-policy.mjs', import.meta.url);
const modulePath = fileURLToPath(moduleUrl);

async function loadModule() {
  vi.resetModules();
  return import('./check-overrides-policy.mjs');
}

const PNPM_OVERRIDES = {
  'basic-ftp': '5.3.1',
  dompurify: '3.4.11',
  'ip-address': '10.2.0',
  uuid: '14.0.0',
};

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

describe('checkOverridesPolicy', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('accepts pnpm-scoped overrides without root overrides', async () => {
    const { checkOverridesPolicy } = await loadModule();

    expect(checkOverridesPolicy({ pnpm: { overrides: PNPM_OVERRIDES } })).toEqual({
      ok: true,
      pnpmOverridesToCheck: ['basic-ftp', 'dompurify', 'ip-address', 'uuid'],
      rootOverrides: [],
      reason: 'matched',
    });
  });

  it('rejects top-level overrides because npm and npx consume them', async () => {
    const { checkOverridesPolicy } = await loadModule();

    expect(
      checkOverridesPolicy({
        overrides: {
          ajv: '8.20.0',
          dompurify: '3.4.11',
        },
        pnpm: { overrides: PNPM_OVERRIDES },
      }),
    ).toMatchObject({
      ok: false,
      rootOverrides: ['ajv', 'dompurify'],
      reason: 'root-overrides-present',
    });
  });

  it('reports missing pnpm overrides', async () => {
    const { checkOverridesPolicy } = await loadModule();

    expect(checkOverridesPolicy({})).toEqual({
      ok: false,
      pnpmOverridesToCheck: [],
      rootOverrides: [],
      reason: 'missing-pnpm-overrides',
    });
  });

  it('reports root overrides before missing pnpm overrides', async () => {
    const { checkOverridesPolicy } = await loadModule();

    expect(checkOverridesPolicy({ overrides: { dompurify: '3.4.11' } })).toMatchObject({
      ok: false,
      pnpmOverridesToCheck: [],
      rootOverrides: ['dompurify'],
      reason: 'root-overrides-present',
    });
  });

  it('keeps the live package manifest npm-compatible', async () => {
    const { checkOverridesPolicy } = await loadModule();

    expect(checkOverridesPolicy(packageJson).ok).toBe(true);
  });
});

describe('reportOverridesPolicy', () => {
  it('logs success and returns 0 for a passing report', async () => {
    const { reportOverridesPolicy } = await loadModule();
    const outputIo = { log: vi.fn(), error: vi.fn() };

    const exitCode = reportOverridesPolicy(
      {
        ok: true,
        pnpmOverridesToCheck: ['basic-ftp', 'dompurify'],
        rootOverrides: [],
        reason: 'matched',
      },
      outputIo,
    );

    expect(exitCode).toBe(0);
    expect(outputIo.log).toHaveBeenCalledWith(
      'pnpm override policy verified for basic-ftp, dompurify.',
    );
    expect(outputIo.error).not.toHaveBeenCalled();
  });

  it('logs root override failures and returns 1', async () => {
    const { reportOverridesPolicy } = await loadModule();
    const outputIo = { log: vi.fn(), error: vi.fn() };

    const exitCode = reportOverridesPolicy(
      {
        ok: false,
        pnpmOverridesToCheck: ['dompurify'],
        rootOverrides: ['ajv', 'dompurify'],
        reason: 'root-overrides-present',
      },
      outputIo,
    );

    expect(exitCode).toBe(1);
    expect(outputIo.error).toHaveBeenCalledWith(
      [
        'Override policy check failed.',
        'Top-level overrides are not allowed because npm and npx consume them.',
        'Move these entries under pnpm.overrides only: ajv, dompurify',
      ].join('\n'),
    );
    expect(outputIo.log).not.toHaveBeenCalled();
  });

  it('logs missing pnpm overrides and returns 1', async () => {
    const { reportOverridesPolicy } = await loadModule();
    const outputIo = { log: vi.fn(), error: vi.fn() };

    const exitCode = reportOverridesPolicy(
      {
        ok: false,
        pnpmOverridesToCheck: [],
        rootOverrides: [],
        reason: 'missing-pnpm-overrides',
      },
      outputIo,
    );

    expect(exitCode).toBe(1);
    expect(outputIo.error).toHaveBeenCalledWith(
      'Override policy check failed.\nNo pnpm.overrides entries were found.',
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
    readFileMock.mockResolvedValueOnce(JSON.stringify({ pnpm: { overrides: PNPM_OVERRIDES } }));
    process.argv = [process.argv[0], modulePath];

    await loadModule();

    expect(readFileMock).toHaveBeenCalledTimes(1);
    expect(process.exitCode).toBe(0);
    expect(consoleLogSpy).toHaveBeenCalledWith(
      'pnpm override policy verified for basic-ftp, dompurify, ip-address, uuid.',
    );
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('sets exitCode and logs failures when invoked directly', async () => {
    readFileMock.mockResolvedValueOnce(
      JSON.stringify({ overrides: { dompurify: '3.4.11' } }),
    );
    process.argv = [process.argv[0], modulePath];

    await loadModule();

    expect(process.exitCode).toBe(1);
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      [
        'Override policy check failed.',
        'Top-level overrides are not allowed because npm and npx consume them.',
        'Move these entries under pnpm.overrides only: dompurify',
      ].join('\n'),
    );
    expect(consoleLogSpy).not.toHaveBeenCalled();
  });
});
