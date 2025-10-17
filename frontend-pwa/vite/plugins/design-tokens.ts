/**
 * @file Vite plugin that ensures the design tokens package is built and aliased.
 */
import type { SpawnSyncReturns } from 'node:child_process';
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
  cwd?: string;
}

const defaultPackageName = '@app/tokens';
const defaultPackagePath = 'packages/tokens';
const defaultDistPath = 'dist';
const defaultAlias = '@app/tokens';

const lockfileLookup = [
  ['pnpm', 'pnpm-lock.yaml'],
  ['yarn', 'yarn.lock'],
  ['bun', 'bun.lockb'],
  ['npm', 'package-lock.json'],
] as const satisfies ReadonlyArray<readonly [PackageManager, string]>;

interface ResolvedPaths {
  packageName: string;
  packagePath: string;
  distPath: string;
}

function resolveTokensPaths(options: EnsureTokensDistOptions): ResolvedPaths {
  const packageName = options.packageName ?? defaultPackageName;
  const packagePath = resolve(
    options.workspaceRoot,
    options.packageRelativePath ?? defaultPackagePath,
  );
  const distPath = resolve(packagePath, options.distRelativePath ?? defaultDistPath);

  return { packageName, packagePath, distPath };
}

function detectFromUserAgent(): PackageManager | null {
  // biome-ignore lint/style/noProcessEnv: environment hints come from npm.
  const agent = process.env.npm_config_user_agent;
  if (!agent) return null;

  const managers: PackageManager[] = ['pnpm', 'yarn', 'bun', 'npm'];
  for (const manager of managers) {
    if (agent.includes(manager)) {
      return manager;
    }
  }

  return null;
}

function detectFromLockfile(workspaceRoot: string): PackageManager | null {
  for (const [manager, lockfile] of lockfileLookup) {
    if (existsSync(resolve(workspaceRoot, lockfile))) {
      return manager;
    }
  }

  return null;
}

export function detectPackageManager(workspaceRoot: string): PackageManager {
  return detectFromUserAgent() ?? detectFromLockfile(workspaceRoot) ?? 'pnpm';
}

function buildFailed(result: SpawnSyncReturns<Buffer>): boolean {
  return Boolean(result.error) || result.status === null || result.status !== 0;
}

function buildFailureMessage(command: BuildCommand, result: SpawnSyncReturns<Buffer>): string {
  const segments = [
    'Design tokens build failed.',
    `Command: ${command.pretty}.`,
    'Check the output above for details.',
    result.error ? `Error: ${result.error.message}` : undefined,
  ];

  return segments.filter((segment): segment is string => Boolean(segment)).join(' ');
}

function buildCommandFor(
  packageManager: PackageManager,
  packageName: string,
  packagePath: string,
): BuildCommand {
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
        args: ['workspace', packageName, 'run', 'build'],
        pretty: `yarn workspace ${packageName} run build`,
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
        args: ['run', 'build'],
        pretty: `bun run build (from ${packageName})`,
        cwd: packagePath,
      };
    default: {
      const exhaustive: never = packageManager;
      throw new Error(`Unsupported package manager: ${exhaustive}`);
    }
  }
}

export function ensureTokensDist(options: EnsureTokensDistOptions): string {
  const { packageName, packagePath, distPath } = resolveTokensPaths(options);

  if (existsSync(distPath)) {
    return distPath;
  }

  const manager = detectPackageManager(options.workspaceRoot);
  const buildCommand = buildCommandFor(manager, packageName, packagePath);
  options.logger.info(`Design tokens dist missing, running \`${buildCommand.pretty}\` to rebuild.`);
  const result = spawnSync(buildCommand.command, buildCommand.args, {
    cwd: buildCommand.cwd ?? options.workspaceRoot,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });

  if (buildFailed(result)) {
    options.logger.error(buildFailureMessage(buildCommand, result));
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

export function designTokensPlugin(options: DesignTokensPluginOptions): Plugin {
  const packageName = options.packageName ?? defaultPackageName;
  const alias = options.alias ?? defaultAlias;
  const packagePath = resolve(
    options.workspaceRoot,
    options.packageRelativePath ?? defaultPackagePath,
  );
  const distPath = resolve(packagePath, options.distRelativePath ?? defaultDistPath);

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
