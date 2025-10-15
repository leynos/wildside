/**
 * @file Vite plugin that ensures the design tokens package is built and aliased.
 */
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Logger, Plugin } from 'vite';

type PackageManager = 'pnpm' | 'yarn' | 'npm' | 'bun';

export interface EnsureTokensDistOptions {
  workspaceRoot: string;
  packageName?: string;
  packageRelativePath?: string;
  distRelativePath?: string;
  logger: Logger;
}

export interface DesignTokensPluginOptions {
  workspaceRoot: string;
  packageName?: string;
  alias?: string;
  packageRelativePath?: string;
  distRelativePath?: string;
}

interface BuildCommand {
  command: string;
  args: string[];
  pretty: string;
}

const DEFAULT_PACKAGE_NAME = '@app/tokens';
const DEFAULT_PACKAGE_PATH = 'packages/tokens';
const DEFAULT_DIST_PATH = 'dist';
const DEFAULT_ALIAS = '@app/tokens';

export function detectPackageManager(workspaceRoot: string): PackageManager {
  const agent = process.env.npm_config_user_agent;
  if (agent?.includes('pnpm')) return 'pnpm';
  if (agent?.includes('yarn')) return 'yarn';
  if (agent?.includes('bun')) return 'bun';
  if (agent?.includes('npm')) return 'npm';

  const lookup: Array<[PackageManager, string]> = [
    ['pnpm', 'pnpm-lock.yaml'],
    ['yarn', 'yarn.lock'],
    ['bun', 'bun.lock'],
    ['npm', 'package-lock.json'],
  ];

  for (const [manager, lockfile] of lookup) {
    if (existsSync(resolve(workspaceRoot, lockfile))) {
      return manager;
    }
  }

  return 'pnpm';
}

function buildCommandFor(packageManager: PackageManager, packageName: string): BuildCommand {
  switch (packageManager) {
    case 'pnpm':
      return {
        command: 'pnpm',
        args: ['--filter', packageName, 'build'],
        pretty: `pnpm --filter ${packageName} build`,
      };
    case 'yarn':
      return {
        command: 'yarn',
        args: ['workspace', packageName, 'build'],
        pretty: `yarn workspace ${packageName} build`,
      };
    case 'npm':
      return {
        command: 'npm',
        args: ['run', 'build', '--workspace', packageName],
        pretty: `npm run build --workspace ${packageName}`,
      };
    case 'bun':
      return {
        command: 'bun',
        args: ['run', '--filter', packageName, 'build'],
        pretty: `bun run --filter ${packageName} build`,
      };
    default: {
      const exhaustive: never = packageManager;
      throw new Error(`Unsupported package manager: ${exhaustive}`);
    }
  }
}

export function ensureTokensDist(options: EnsureTokensDistOptions): string {
  const packageName = options.packageName ?? DEFAULT_PACKAGE_NAME;
  const packagePath = resolve(
    options.workspaceRoot,
    options.packageRelativePath ?? DEFAULT_PACKAGE_PATH,
  );
  const distPath = resolve(packagePath, options.distRelativePath ?? DEFAULT_DIST_PATH);

  if (existsSync(distPath)) {
    return distPath;
  }

  const manager = detectPackageManager(options.workspaceRoot);
  const buildCommand = buildCommandFor(manager, packageName);
  options.logger.info(
    `Design tokens dist missing, running \`${buildCommand.pretty}\` to rebuild.`,
  );
  const result = spawnSync(buildCommand.command, buildCommand.args, {
    cwd: options.workspaceRoot,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });

  if (result.status !== 0) {
    options.logger.error(
      [
        'Design tokens build failed.',
        `Command: ${buildCommand.pretty}.`,
        'Check the output above for details.',
      ].join(' '),
    );
    throw new Error('Design tokens build failed.');
  }

  if (!existsSync(distPath)) {
    options.logger.error(
      [
        'Design tokens build completed but the dist directory is still missing.',
        'Ensure the build output path is correct.',
      ].join(' '),
    );
    throw new Error('Design tokens dist not found after build.');
  }

  options.logger.info('Design tokens dist ready.');
  return distPath;
}

export function designTokensPlugin(
  options: DesignTokensPluginOptions,
): Plugin {
  const packageName = options.packageName ?? DEFAULT_PACKAGE_NAME;
  const alias = options.alias ?? DEFAULT_ALIAS;
  const packagePath = resolve(
    options.workspaceRoot,
    options.packageRelativePath ?? DEFAULT_PACKAGE_PATH,
  );
  const distPath = resolve(packagePath, options.distRelativePath ?? DEFAULT_DIST_PATH);

  return {
    name: 'wildside-design-tokens',
    enforce: 'pre',
    config: () => ({
      resolve: {
        alias: {
          [alias]: distPath,
        },
      },
    }),
    configResolved(resolvedConfig) {
      ensureTokensDist({
        workspaceRoot: options.workspaceRoot,
        packageName,
        packageRelativePath: options.packageRelativePath,
        distRelativePath: options.distRelativePath,
        logger: resolvedConfig.logger,
      });
    },
  };
}
