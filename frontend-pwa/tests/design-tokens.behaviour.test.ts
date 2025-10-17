/**
 * @file Behavioural tests for the design tokens Vite plugin.
 */

import type { Plugin, ResolvedConfig } from 'vite';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('node:fs', () => ({
  existsSync: vi.fn(),
}));

vi.mock('node:child_process', () => ({
  spawnSync: vi.fn(),
}));

const { existsSync } = await import('node:fs');
const { spawnSync } = await import('node:child_process');
const { resolve } = await import('node:path');
const { designTokensPlugin } = await import('../vite/plugins/design-tokens');
const { pathToString } = await import('./test-helpers');
const { createMockLogger } = await import('./test-logger');

const existsSyncMock = vi.mocked(existsSync);
const spawnSyncMock = vi.mocked(spawnSync);

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
    // biome-ignore lint/style/noProcessEnv: tests simulate npm CLI hints.
    delete process.env.npm_config_user_agent;
  });

  it('exposes a @app/tokens alias from the config hook', async () => {
    const plugin = designTokensPlugin({ workspaceRoot });
    const config = (await invokeConfigHook(plugin)) as {
      resolve?: { alias?: Record<string, string> };
    };
    expect(config.resolve?.alias?.['@app/tokens']).toBe(distPath);
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
