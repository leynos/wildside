/**
 * @file Unit tests for the design tokens plugin utilities.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Logger } from 'vite';
import { ensureTokensDist, detectPackageManager } from '../vite/plugins/designTokens';
import { existsSync } from 'node:fs';
import type { PathLike } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { createMockLogger } from './testLogger';

vi.mock('node:fs', () => ({
  existsSync: vi.fn(),
}));

vi.mock('node:child_process', () => ({
  spawnSync: vi.fn(),
}));

const existsSyncMock = vi.mocked(existsSync);
const spawnSyncMock = vi.mocked(spawnSync);

function pathToString(path: PathLike): string {
  return typeof path === 'string' ? path : path.toString();
}

describe('detectPackageManager', () => {
  const workspaceRoot = '/workspace/project';

  beforeEach(() => {
    vi.resetAllMocks();
    delete process.env.npm_config_user_agent;
  });

  it('prefers npm_config_user_agent hints when available', () => {
    process.env.npm_config_user_agent = 'pnpm/9.0.0 npm/? node/?';
    expect(detectPackageManager(workspaceRoot)).toBe('pnpm');

    process.env.npm_config_user_agent = 'yarn/4.0.0 npm/? node/?';
    expect(detectPackageManager(workspaceRoot)).toBe('yarn');
  });

  it('falls back to lockfile discovery', () => {
    existsSyncMock.mockImplementation((path) =>
      pathToString(path).endsWith('yarn.lock'),
    );
    expect(detectPackageManager(workspaceRoot)).toBe('yarn');
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

  it('returns immediately when the dist directory already exists', () => {
    existsSyncMock.mockImplementation((path) => pathToString(path) === distPath);

    expect(ensureTokensDist({ workspaceRoot, logger })).toBe(distPath);
    expect(spawnSyncMock).not.toHaveBeenCalled();
  });

  it('builds the tokens package when the dist directory is missing', () => {
    let distExists = false;
    existsSyncMock.mockImplementation((path) => {
      const target = pathToString(path);
      if (target.endsWith('pnpm-lock.yaml')) return true;
      if (target === distPath) return distExists;
      return false;
    });
    spawnSyncMock.mockImplementation(() => {
      distExists = true;
      return { status: 0 } as ReturnType<typeof spawnSync>;
    });

    expect(ensureTokensDist({ workspaceRoot, logger })).toBe(distPath);
    expect(spawnSyncMock).toHaveBeenCalledWith(
      'pnpm',
      ['--filter', '@app/tokens', 'build'],
      expect.objectContaining({ cwd: workspaceRoot }),
    );
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
