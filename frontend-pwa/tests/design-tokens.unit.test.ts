/**
 * @file Unit tests for the design tokens plugin utilities.
 */

// biome-ignore assist/source/organizeImports: maintain external/node/local grouping required by review.
import type { Logger } from 'vite';
import { beforeEach, describe, expect, it, mock } from 'bun:test';

import type { spawnSync } from 'node:child_process';
import type { PathLike } from 'node:fs';
import { resolve } from 'node:path';

import { detectPackageManager, ensureTokensDist } from '../vite/plugins/design-tokens';
import { pathToString } from './test-helpers';
import { createMockLogger } from './test-logger';

const existsSyncMock = mock();
const spawnSyncMock = mock();

mock.module('node:fs', () => ({
  existsSync: existsSyncMock,
}));

mock.module('node:child_process', () => ({
  spawnSync: spawnSyncMock,
}));

describe('detectPackageManager', () => {
  const workspaceRoot = '/workspace/project';

  beforeEach(() => {
    existsSyncMock.mockReset();
    spawnSyncMock.mockReset();
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
    existsSyncMock.mockImplementation((path: PathLike) => pathToString(path).endsWith('yarn.lock'));
    expect(detectPackageManager(workspaceRoot)).toBe('yarn');
  });

  it('falls back to npm lockfile discovery when user agent is missing', () => {
    existsSyncMock.mockImplementation((path: PathLike) =>
      pathToString(path).endsWith('package-lock.json'),
    );
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
    existsSyncMock.mockReset();
    spawnSyncMock.mockReset();
    // biome-ignore lint/style/noProcessEnv: tests simulate npm CLI hints.
    delete process.env.npm_config_user_agent;
    logger = createMockLogger();
  });

  /**
   * Configures the dist lookup to simulate a missing build whilst keeping the
   * package manager detection lockfile available.
   *
   * @param lockfileCheck - Optional matcher for lockfile discovery.
   */
  function mockDistMissing(lockfileCheck?: (path: string) => boolean): void {
    existsSyncMock.mockImplementation((path: PathLike) => {
      const target = pathToString(path);
      if (lockfileCheck?.(target)) return true;
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
    packageManager: 'pnpm' | 'yarn' | 'npm' | 'bun';
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

    existsSyncMock.mockImplementation((path: PathLike) => {
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

    existsSyncMock.mockReset();
    spawnSyncMock.mockReset();
  }

  it('returns immediately when the dist directory already exists', () => {
    existsSyncMock.mockImplementation((path: PathLike) => pathToString(path) === distPath);

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

  it('runs the npm workspace build script when npm is detected', () => {
    testPackageManagerBuild({
      packageManager: 'npm',
      userAgent: 'npm/10.0.0 node/?',
      expectedCommand: 'npm',
      expectedArgs: ['run', 'build', '--workspace', '@app/tokens'],
      expectedCwd: workspaceRoot,
    });
  });

  it('throws when the build command fails', () => {
    mockDistMissing((path) => path.endsWith('pnpm-lock.yaml'));
    spawnSyncMock.mockReturnValue({ status: 1 } as ReturnType<typeof spawnSync>);

    expect(() => ensureTokensDist({ workspaceRoot, logger })).toThrow(
      'Design tokens build failed.',
    );
  });

  it('throws when the dist directory is still missing after a successful build', () => {
    mockDistMissing((path) => path.endsWith('pnpm-lock.yaml'));
    spawnSyncMock.mockReturnValue({ status: 0 } as ReturnType<typeof spawnSync>);

    expect(() => ensureTokensDist({ workspaceRoot, logger })).toThrow(
      'Design tokens dist not found after build.',
    );
  });
});
