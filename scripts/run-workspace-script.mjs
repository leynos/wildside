#!/usr/bin/env node
/**
 * @file Runs a named workspace script across every package without recursing into
 * the root aggregator.
 *
 * The earlier approach relied on `bun run <script> --workspaces`, which re-entered
 * the root package because Bun treats the aggregator as part of the workspace
 * graph. This helper enumerates packages via `pnpm`, skips the root manifest, and
 * delegates to `bun run` only when the target script exists. Any arguments after
 * a literal `--` are forwarded; if the separator is absent, every additional
 * argument is passed through verbatim. The guards prevent infinite recursion and
 * avoid noisy "script not found" errors for packages that do not yet expose the
 * requested entry point.
 */

import { execFile, spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);

const [, , maybeScriptName, ...rawArgs] = process.argv;

if (!maybeScriptName || maybeScriptName === '-h' || maybeScriptName === '--help') {
  console.error('Usage: node scripts/run-workspace-script.mjs <script> [args…]');
  process.exit(maybeScriptName ? 0 : 1);
}

const separatorIndex = rawArgs.indexOf('--');
const forwardedArgs = separatorIndex === -1 ? rawArgs : rawArgs.slice(separatorIndex + 1);

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

async function getWorkspacePackages() {
  const rootPackage = JSON.parse(await readFile(path.join(repoRoot, 'package.json'), 'utf8'));

  const { stdout } = await execFileAsync('pnpm', ['-r', 'ls', '--depth', '-1', '--json'], {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 10 * 1024 * 1024, // 10MB for large monorepos
  });

  const workspaceEntries = JSON.parse(stdout);

  return workspaceEntries
    .filter((entry) => path.resolve(entry.path) !== repoRoot)
    .filter((entry) => entry.name !== rootPackage.name)
    .sort((a, b) => a.path.localeCompare(b.path));
}

async function loadPackageManifest(manifestPath, pkgPath) {
  try {
    return JSON.parse(await readFile(manifestPath, 'utf8'));
  } catch (error) {
    console.warn(`Skipping ${pkgPath}: unable to read package manifest (${error.message}).`);
    return null;
  }
}

async function runScriptInPackage(scriptName, args, pkgPath) {
  const exitMeta = await new Promise((resolve) => {
    const child = spawn('bun', ['run', scriptName, ...args], {
      cwd: pkgPath,
      stdio: 'inherit',
      env: process.env,
    });

    child.on('error', (error) => {
      console.error(`Failed to start bun in ${pkgPath}: ${error.message}`);
      resolve({ code: 1, signal: null });
    });

    child.on('exit', (code, signal) => {
      resolve({ code, signal });
    });
  });

  if (exitMeta.signal) {
    console.error(`bun run ${scriptName} terminated by signal ${exitMeta.signal} in ${pkgPath}`);
    process.exit(1);
  }

  if (exitMeta.code !== 0) {
    process.exit(exitMeta.code ?? 1);
  }
}

async function main() {
  const workspacePackages = await getWorkspacePackages();
  let invokedCount = 0;

  for (const pkg of workspacePackages) {
    const manifestPath = path.join(pkg.path, 'package.json');
    const manifest = await loadPackageManifest(manifestPath, pkg.path);

    if (!manifest) {
      continue;
    }

    if (!manifest.scripts || !(maybeScriptName in manifest.scripts)) {
      continue;
    }

    invokedCount += 1;
    const displayArgs = forwardedArgs.length ? ` ${forwardedArgs.join(' ')}` : '';
    console.log(`\n[workspace:${manifest.name}] bun run ${maybeScriptName}${displayArgs}`);

    await runScriptInPackage(maybeScriptName, forwardedArgs, pkg.path);
  }

  if (invokedCount === 0) {
    console.log(`No workspace package defines the "${maybeScriptName}" script. Nothing to run.`);
  }
}

main().catch((error) => {
  console.error(`Failed to execute workspace script: ${error.message}`);
  process.exit(1);
});
