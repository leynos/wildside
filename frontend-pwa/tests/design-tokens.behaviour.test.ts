/**
 * @file Behavioural tests for the design tokens Vite plugin.
 */

// biome-ignore assist/source/organizeImports: maintain external/node/local grouping required by review.
import type {
  ConfigEnv,
  ConfigPluginContext,
  Logger,
  MinimalPluginContextWithoutEnvironment,
  Plugin,
  ResolvedConfig,
  UserConfig,
} from 'vite';
import { beforeEach, describe, expect, it, mock } from 'bun:test';

import type { spawnSync } from 'node:child_process';
import type { PathLike } from 'node:fs';
import path from 'node:path';

import { designTokensPlugin } from '../vite/plugins/design-tokens';
import { createMockLogger } from './test-logger';

const existsSyncMock = mock();
const spawnSyncMock = mock();

await mock.module('node:fs', () => ({
  existsSync: existsSyncMock,
}));

await mock.module('node:child_process', () => ({
  spawnSync: spawnSyncMock,
}));

function invokeConfigHook(plugin: Plugin) {
  const hook = plugin.config;
  if (!hook) return;
  const initialConfig: UserConfig = {};
  const env: ConfigEnv = { command: 'serve', mode: 'development' };
  if ('handler' in hook) {
    // SAFETY: This plugin's config hook closes over its dependencies and does not read Vite context.
    return hook.handler.call({} as ConfigPluginContext, initialConfig, env);
  }
  // SAFETY: This plugin's config hook closes over its dependencies and does not read Vite context.
  return hook.call({} as ConfigPluginContext, initialConfig, env);
}

function assertResolvedConfig(config: Partial<ResolvedConfig>): void {
  if (!config.logger) {
    throw new Error('Resolved config mock must define a logger.');
  }
}

function invokeConfigResolved(plugin: Plugin, resolvedConfig: Partial<ResolvedConfig>) {
  const hook = plugin.configResolved;
  if (!hook) return;
  assertResolvedConfig(resolvedConfig);
  const handler = 'handler' in hook ? hook.handler : hook;
  // SAFETY: The design tokens hook reads only logger, which this fixture verifies above.
  return handler.call(
    {} as MinimalPluginContextWithoutEnvironment,
    resolvedConfig as ResolvedConfig,
  );
}

/**
 * Creates a minimal resolved config with a mock logger by default so tests can
 * override the logger when verifying error reporting behaviour.
 */
function createResolvedConfig(logger?: Logger): Partial<ResolvedConfig> {
  return {
    logger: logger ?? createMockLogger(),
  } satisfies Partial<ResolvedConfig>;
}

describe('designTokensPlugin', () => {
  const workspaceRoot = '/workspace/project';
  const distPath = path.resolve(workspaceRoot, 'packages/tokens/dist');

  beforeEach(() => {
    existsSyncMock.mockReset();
    spawnSyncMock.mockReset();
    // biome-ignore lint/style/noProcessEnv: tests simulate npm CLI hints.
    delete process.env.npm_config_user_agent;
  });

  /**
   * Configures the filesystem mock to simulate a missing design tokens dist
   * directory whilst leaving the pnpm lockfile visible. Tests can flip the
   * dist state via the returned controller to emulate rebuild outcomes.
   */
  function mockDistMissing() {
    let distExists = false;
    const fallback = (path: PathLike) => {
      const target = String(path);
      if (target.endsWith('pnpm-lock.yaml')) return true;
      if (target === distPath) return distExists;
      return false;
    };
    existsSyncMock.mockImplementation(fallback);
    return {
      setDistExists(value: boolean) {
        distExists = value;
      },
    };
  }

  it('exposes a @app/tokens alias from the config hook', async () => {
    const plugin = designTokensPlugin({ workspaceRoot });
    const config = await invokeConfigHook(plugin);
    expect(config).toMatchObject({ resolve: { alias: { '@app/tokens': distPath } } });
  });

  it('runs the build when the dist directory is missing', () => {
    const distController = mockDistMissing();
    spawnSyncMock.mockImplementation(() => {
      distController.setDistExists(true);
      // SAFETY: The plugin reads only the mocked child process status and error fields.
      return { status: 0 } as ReturnType<typeof spawnSync>;
    });

    const plugin = designTokensPlugin({ workspaceRoot });
    const resolvedConfig = createResolvedConfig();
    const result = invokeConfigResolved(plugin, resolvedConfig);

    expect(result).toBeUndefined();

    expect(spawnSyncMock).toHaveBeenCalledWith(
      'pnpm',
      ['--filter', '@app/tokens', 'build'],
      expect.objectContaining({ cwd: workspaceRoot }),
    );
  });

  it('throws when the rebuild fails', () => {
    mockDistMissing();
    const logger = createMockLogger();
    // SAFETY: The plugin reads only the mocked child process status and error fields.
    spawnSyncMock.mockReturnValue({ status: 1 } as ReturnType<typeof spawnSync>);

    const plugin = designTokensPlugin({ workspaceRoot });
    const resolvedConfig = createResolvedConfig(logger);

    expect(() => invokeConfigResolved(plugin, resolvedConfig)).toThrow(
      'Design tokens build failed.',
    );
    expect(logger.error).toHaveBeenCalled();
  });
});
