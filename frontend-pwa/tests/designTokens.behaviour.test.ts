/**
 * @file Behavioural tests for the design tokens Vite plugin.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ResolvedConfig } from 'vite';
import { designTokensPlugin } from '../vite/plugins/designTokens';
import { existsSync } from 'node:fs';
import type { PathLike } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { createMockLogger } from './testLogger';
import type { Plugin } from 'vite';

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

async function invokeConfigHook(plugin: Plugin) {
  const hook = plugin.config;
  if (!hook) return undefined;
  if (typeof hook === 'function') {
    return (hook as unknown as (config: unknown, env: unknown) => unknown)(
      {},
      { command: 'serve', mode: 'development' },
    );
  }
  return (hook.handler as unknown as (config: unknown, env: unknown) => unknown)(
    {},
    { command: 'serve', mode: 'development' },
  );
}

function invokeConfigResolved(plugin: Plugin, resolvedConfig: ResolvedConfig) {
  const hook = plugin.configResolved;
  if (!hook) return;
  if (typeof hook === 'function') {
    return (hook as unknown as (config: ResolvedConfig) => unknown)(resolvedConfig);
  }
  return (hook.handler as unknown as (config: ResolvedConfig) => unknown)(resolvedConfig);
}

function createResolvedConfig(): ResolvedConfig {
  return {
    logger: createMockLogger(),
  } as unknown as ResolvedConfig;
}

describe('designTokensPlugin', () => {
  const workspaceRoot = '/workspace/project';
  const distPath = resolve(workspaceRoot, 'packages/tokens/dist');

  beforeEach(() => {
    vi.resetAllMocks();
    existsSyncMock.mockReset();
    spawnSyncMock.mockReset();
    delete process.env.npm_config_user_agent;
  });

  it('exposes a @app/tokens alias from the config hook', async () => {
    const plugin = designTokensPlugin({ workspaceRoot });
    const config = (await invokeConfigHook(plugin)) as any;
    expect(config?.resolve?.alias?.['@app/tokens']).toBe(distPath);
  });

  it('runs the build when the dist directory is missing', () => {
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

    const plugin = designTokensPlugin({ workspaceRoot });
    const resolvedConfig = createResolvedConfig();
    invokeConfigResolved(plugin, resolvedConfig);

    expect(spawnSyncMock).toHaveBeenCalledWith(
      'pnpm',
      ['--filter', '@app/tokens', 'build'],
      expect.objectContaining({ cwd: workspaceRoot }),
    );
  });

  it('throws when the rebuild fails', () => {
    existsSyncMock.mockImplementation((path) => {
      const target = pathToString(path);
      if (target.endsWith('pnpm-lock.yaml')) return true;
      return false;
    });
    spawnSyncMock.mockReturnValue({ status: 1 } as ReturnType<typeof spawnSync>);

    const plugin = designTokensPlugin({ workspaceRoot });
    const resolvedConfig = createResolvedConfig();

    expect(() => invokeConfigResolved(plugin, resolvedConfig)).toThrow(
      'Design tokens build failed.',
    );
  });
});
