/**
 * @file Behavioural tests for the design tokens Vite plugin.
 */

// biome-ignore assist/source/organizeImports: maintain external/node/local grouping required by review.
import type {
  ConfigEnv,
  ConfigPluginContext,
  Logger,
  Plugin,
  ResolvedConfig,
  UserConfig,
} from 'vite';
import { beforeEach, describe, expect, it, mock } from 'bun:test';

import type { spawnSync } from 'node:child_process';
import type { PathLike } from 'node:fs';
import { resolve } from 'node:path';

import { designTokensPlugin } from '../vite/plugins/design-tokens';
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

type ConfigHook = NonNullable<Plugin['config']>;
type ConfigHookHandler = Extract<
  ConfigHook,
  (this: ConfigPluginContext, config: UserConfig, env: ConfigEnv) => unknown
>;
type ConfigHookDescriptor = Extract<
  ConfigHook,
  { handler: (this: ConfigPluginContext, config: UserConfig, env: ConfigEnv) => unknown }
>;

function isConfigHookHandler(hook: Plugin['config']): hook is ConfigHookHandler {
  return typeof hook === 'function';
}

function isConfigHookDescriptor(hook: Plugin['config']): hook is ConfigHookDescriptor {
  return (
    typeof hook === 'object' &&
    hook !== null &&
    'handler' in hook &&
    typeof hook.handler === 'function'
  );
}

async function invokeConfigHook(plugin: Plugin) {
  const hook = plugin.config;
  if (!hook) return undefined;
  const initialConfig = {} satisfies UserConfig;
  const env: ConfigEnv = { command: 'serve', mode: 'development' };
  if (isConfigHookHandler(hook)) {
    return hook.bind({} as ConfigPluginContext)(initialConfig, env);
  }
  if (isConfigHookDescriptor(hook)) {
    return hook.handler.bind({} as ConfigPluginContext)(initialConfig, env);
  }
  return hook;
}

type ConfigResolvedHook = (config: ResolvedConfig) => void | Promise<void>;

function assertResolvedConfig(config: Partial<ResolvedConfig>): asserts config is ResolvedConfig {
  if (!config.logger) {
    throw new Error('Resolved config mock must define a logger.');
  }
}

function invokeConfigResolved(plugin: Plugin, resolvedConfig: Partial<ResolvedConfig>) {
  const hook = plugin.configResolved;
  if (!hook) return;
  assertResolvedConfig(resolvedConfig);
  const handler: ConfigResolvedHook = typeof hook === 'function' ? hook : hook.handler;
  return handler(resolvedConfig);
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
  const distPath = resolve(workspaceRoot, 'packages/tokens/dist');

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
      const target = pathToString(path);
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
    const config = (await invokeConfigHook(plugin)) as {
      resolve?: { alias?: Record<string, string> };
    };
    expect(config.resolve?.alias?.['@app/tokens']).toBe(distPath);
  });

  it('runs the build when the dist directory is missing', () => {
    const distController = mockDistMissing();
    spawnSyncMock.mockImplementation(() => {
      distController.setDistExists(true);
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
    spawnSyncMock.mockReturnValue({ status: 1 } as ReturnType<typeof spawnSync>);

    const plugin = designTokensPlugin({ workspaceRoot });
    const resolvedConfig = createResolvedConfig(logger);

    expect(() => invokeConfigResolved(plugin, resolvedConfig)).toThrow(
      'Design tokens build failed.',
    );
    expect(logger.error).toHaveBeenCalled();
  });
});
