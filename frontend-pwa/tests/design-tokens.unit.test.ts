/**
 * @file Unit tests for the design tokens plugin utilities.
 */

import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Logger } from 'vite';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { detectPackageManager, ensureTokensDist } from '../vite/plugins/design-tokens';
import { pathToString } from './test-helpers';
import { createMockLogger } from './test-logger';

vi.mock('node:fs', () => ({
  existsSync: vi.fn(),
}));

vi.mock('node:child_process', () => ({
  spawnSync: vi.fn(),
}));

const existsSyncMock = vi.mocked(existsSync);
const spawnSyncMock = vi.mocked(spawnSync);

describe('detectPackageManager', () => {
  const workspaceRoot = '/workspace/project';

  beforeEach(() => {
    vi.resetAllMocks();
    // biome-ignore lint/style/noProcessEnv: tests simulate npm CLI hints.
    delete process.env.npm_config_user_agent;
  });

  it('detects pnpm from npm_config_user_agent hints when available', () => {
    // biome-ignore lint/style/noProcessEnv: tests simulate npm CLI hints.
    process.env.npm_config_user_agent = 'pnpm/9.0.0 npm/? node/?';
    expect(detectPackageManager(workspaceRoot)).toBe('pnpm');
  });

  it('detects yarn from npm_config_user_agent hints when available', () => {
    // biome-ignore lint/style/noProcessEnv: tests simulate npm CLI hints.
    process.env.npm_config_user_agent = 'yarn/4.0.0 npm/? node/?';
    expect(detectPackageManager(workspaceRoot)).toBe('yarn');
  });

  it('detects npm from npm_config_user_agent hints when available', () => {
    // biome-ignore lint/style/noProcessEnv: tests simulate npm CLI hints.
    process.env.npm_config_user_agent = 'npm/10.0.0 node/?';
    expect(detectPackageManager(workspaceRoot)).toBe('npm');
  });

  it('falls back to yarn lockfile discovery when user agent is missing', () => {
    existsSyncMock.mockImplementation((path) => pathToString(path).endsWith('yarn.lock'));
    expect(detectPackageManager(workspaceRoot)).toBe('yarn');
  });

  it('falls back to npm lockfile discovery when user agent is missing', () => {
    existsSyncMock.mockImplementation((path) => pathToString(path).endsWith('package-lock.json'));
    expect(detectPackageManager(workspaceRoot)).toBe('npm');
  });

  it('defaults to pnpm when nothing matches', () => {
    existsSyncMock.mockReturnValue(false);
    expect(detectPackageManager(workspaceRoot)).toBe('pnpm');
  });
});

describe('ensureTokensDist', () => {
  const workspaceRoot = '/workspace/project';
  const distPath = resolve(workspaceRoot, 'packages/tokens/dist');
  let logger: Logger;

  beforeEach(() => {
    vi.resetAllMocks();
    // biome-ignore lint/style/noProcessEnv: tests simulate npm CLI hints.
    delete process.env.npm_config_user_agent;
    existsSyncMock.mockReset();
    spawnSyncMock.mockReset();
    logger = createMockLogger();
  });

  /**
   * Configures the dist lookup to simulate a missing build whilst keeping the
   * package manager detection lockfile available.
   */
  function mockDistMissing(): void {
    existsSyncMock.mockImplementation((path) => {
      const target = pathToString(path);
      if (target.endsWith('pnpm-lock.yaml')) return true;
      return false;
    });
  }

  /**
   * Exercises the happy-path build flow for a specific package manager.
   *
   * @param options.packageManager - The manager expected to be detected.
   * @param options.userAgent - Optional CLI user agent hint to seed detection.
   * @param options.lockfileCheck - Optional matcher for lockfile discovery.
   * @param options.expectedCommand - The binary the build should invoke.
   * @param options.expectedArgs - Arguments forwarded to the build command.
   * @param options.expectedCwd - Working directory expected for the spawn call.
   */
  function testPackageManagerBuild(options: {
    packageManager: string;
    userAgent?: string;
    lockfileCheck?: (path: string) => boolean;
    expectedCommand: string;
    expectedArgs: string[];
    expectedCwd: string;
  }): void {
    const { packageManager, userAgent, lockfileCheck, expectedCommand, expectedArgs, expectedCwd } =
      options;
    let distExists = false;

    if (userAgent) {
      // biome-ignore lint/style/noProcessEnv: tests simulate npm CLI hints.
      process.env.npm_config_user_agent = userAgent;
    }

    existsSyncMock.mockImplementation((path) => {
      const target = pathToString(path);
      if (lockfileCheck?.(target)) return true;
      if (target === distPath) return distExists;
      return false;
    });

    spawnSyncMock.mockImplementation(() => {
      distExists = true;
      return { status: 0 } as ReturnType<typeof spawnSync>;
    });

    expect(detectPackageManager(workspaceRoot)).toBe(packageManager);
    expect(ensureTokensDist({ workspaceRoot, logger })).toBe(distPath);
    expect(spawnSyncMock).toHaveBeenCalledWith(
      expectedCommand,
      expectedArgs,
      expect.objectContaining({ cwd: expectedCwd }),
    );
  }

  it('returns immediately when the dist directory already exists', () => {
    existsSyncMock.mockImplementation((path) => pathToString(path) === distPath);

    expect(ensureTokensDist({ workspaceRoot, logger })).toBe(distPath);
    expect(spawnSyncMock).not.toHaveBeenCalled();
  });

  it('builds the tokens package when the dist directory is missing', () => {
    testPackageManagerBuild({
      packageManager: 'pnpm',
      lockfileCheck: (path) => path.endsWith('pnpm-lock.yaml'),
      expectedCommand: 'pnpm',
      expectedArgs: ['--filter', '@app/tokens', 'build'],
      expectedCwd: workspaceRoot,
    });
  });

  it('runs the bun build from the package directory when bun is detected', () => {
    const packagePath = resolve(workspaceRoot, 'packages/tokens');
    testPackageManagerBuild({
      packageManager: 'bun',
      userAgent: 'bun/1.0.0 npm/? node/?',
      expectedCommand: 'bun',
      expectedArgs: ['run', 'build'],
      expectedCwd: packagePath,
    });
  });

  it('runs the yarn workspace build script via yarn run when yarn is detected', () => {
    testPackageManagerBuild({
      packageManager: 'yarn',
      userAgent: 'yarn/4.0.0 npm/? node/?',
      lockfileCheck: (path) => path.endsWith('yarn.lock'),
      expectedCommand: 'yarn',
      expectedArgs: ['workspace', '@app/tokens', 'run', 'build'],
      expectedCwd: workspaceRoot,
    });
  });

  it('throws when the build command fails', () => {
    mockDistMissing();
    spawnSyncMock.mockReturnValue({ status: 1 } as ReturnType<typeof spawnSync>);

    expect(() => ensureTokensDist({ workspaceRoot, logger })).toThrow(
      'Design tokens build failed.',
    );
  });

  it('throws when the dist directory is still missing after a successful build', () => {
    mockDistMissing();
    spawnSyncMock.mockReturnValue({ status: 0 } as ReturnType<typeof spawnSync>);

    expect(() => ensureTokensDist({ workspaceRoot, logger })).toThrow(
      'Design tokens dist not found after build.',
    );
  });
});
