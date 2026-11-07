/**
 * @file Ensures the browser builds Mermaid CLI needs are in Puppeteer's cache.
 *
 * `backend/src/er_snapshots.rs` renders its SVG snapshots through
 * `pnpm exec mmdc`, and Mermaid CLI drives Puppeteer. A developer machine or
 * CI runner can start with an empty Puppeteer cache, so this script downloads
 * the pinned `chrome` and `chrome-headless-shell` builds through Puppeteer's
 * public `browsers install` command.
 *
 * The command is reached through the installed `puppeteer` package's own `bin`
 * entry rather than a package-internal module path. Puppeteer's internal
 * layout is not a public entry point and moves between majors; the `bin`
 * entry is the interface the package publishes.
 *
 * `make nixie` does not need this script. Nixie renders through Merman, and CI
 * installs the pinned `nixie` and `merman-cli` commands before it runs.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';

import { executablePath } from 'puppeteer';

import { isExecutedDirectly } from './direct-execution.mjs';

const MISSING_BROWSER_MESSAGE_FRAGMENT = 'Could not find Chrome';

const require = createRequire(import.meta.url);

/**
 * Reports whether Puppeteer can already resolve an installed browser.
 *
 * Puppeteer signals an absent download by throwing an error that names the
 * missing Chrome build. Any other error is a broken installation rather than
 * a cold cache, so it is rethrown instead of triggering a download that would
 * not address it.
 *
 * `executablePath()` became asynchronous in Puppeteer 25, so the lookup is
 * awaited rather than assumed to return a string.
 *
 * @param {{
 *   resolveExecutable?: () => string | Promise<string>,
 *   isFile?: (path: string) => boolean,
 * }} [options] - Overridable lookup used by the tests.
 * @returns {Promise<boolean>} True when the resolved executable exists on disk.
 */
export async function hasLocalBrowser({
  resolveExecutable = executablePath,
  isFile = existsSync,
} = {}) {
  let browserPath;
  try {
    browserPath = await resolveExecutable();
  } catch (error) {
    if (
      error instanceof Error &&
      error.message.includes(MISSING_BROWSER_MESSAGE_FRAGMENT)
    ) {
      return false;
    }

    throw error;
  }

  return (
    typeof browserPath === 'string' &&
    browserPath.length > 0 &&
    isFile(browserPath)
  );
}

/**
 * Resolves the CLI entry point published by the installed `puppeteer` package.
 *
 * The manifest's `bin` field is either a string or a map keyed by command
 * name, depending on the Puppeteer major, so both shapes are accepted.
 *
 * @param {{
 *   resolvePackageJson?: () => string,
 *   readFile?: (path: string) => string,
 * }} [options] - Overridable resolution used by the tests.
 * @returns {string} The absolute path to Puppeteer's CLI script.
 * @throws {Error} If the installed package publishes no usable `bin` entry.
 */
export function resolvePuppeteerCli({
  resolvePackageJson = () => require.resolve('puppeteer/package.json'),
  readFile = (path) => readFileSync(path, 'utf8'),
} = {}) {
  const manifestPath = resolvePackageJson();
  const manifest = JSON.parse(readFile(manifestPath));
  const bin = manifest?.bin;
  const relativePath = typeof bin === 'string' ? bin : bin?.puppeteer;

  if (typeof relativePath !== 'string' || relativePath.length === 0) {
    throw new Error(
      'The installed puppeteer package publishes no bin entry to invoke.',
    );
  }

  return resolve(dirname(manifestPath), relativePath);
}

/**
 * Runs `puppeteer browsers install`, which downloads every pinned browser.
 *
 * @param {{
 *   cliPath?: string,
 *   execPath?: string,
 *   run?: typeof spawnSync,
 * }} [options] - Overridable invocation used by the tests.
 * @returns {void}
 * @throws {Error} If the install command fails or is terminated by a signal.
 */
export function installPinnedBrowsers({
  cliPath = resolvePuppeteerCli(),
  execPath = process.execPath,
  run = spawnSync,
} = {}) {
  const command = [requireCliPath(cliPath), 'browsers', 'install'];
  assertInstallSucceeded(run(execPath, command, { stdio: 'inherit' }));
}

/**
 * Returns the CLI path to invoke, or explains why it cannot be invoked.
 *
 * @param {string} cliPath - The resolved path to Puppeteer's CLI script.
 * @returns {string} The same path, once it is known to be usable.
 * @throws {Error} If no path was resolved.
 */
function requireCliPath(cliPath) {
  if (typeof cliPath !== 'string' || cliPath.length === 0) {
    throw new Error('Cannot install Puppeteer browsers without a CLI path.');
  }

  return cliPath;
}

/**
 * Throws unless `puppeteer browsers install` completed successfully.
 *
 * The three failure modes stay distinct because they need different fixes: a
 * process that could not start, one killed by a signal, and one that ran and
 * exited non-zero.
 *
 * @param {{
 *   error?: Error,
 *   signal?: string | null,
 *   status: number | null,
 * }} result - The value `spawnSync` returned.
 * @returns {void}
 * @throws {Error} If the command failed to start, was signalled, or exited
 *   with a non-zero status.
 */
function assertInstallSucceeded(result) {
  if (result.error) {
    throw result.error;
  }

  if (result.signal) {
    throw new Error(
      `puppeteer browsers install was terminated by signal ${result.signal}.`,
    );
  }

  if (result.status !== 0) {
    throw new Error(
      `puppeteer browsers install exited with status ${result.status}.`,
    );
  }
}

/**
 * Downloads the pinned browsers only when Puppeteer cannot find one.
 *
 * @param {{
 *   checkBrowser?: () => boolean | Promise<boolean>,
 *   install?: () => void,
 * }} [options] - Overridable steps used by the tests.
 * @returns {Promise<boolean>} True when a download ran.
 * @throws {Error} If the browser is still missing after a successful install.
 */
export async function ensureBrowsersInstalled({
  checkBrowser = () => hasLocalBrowser(),
  install = () => installPinnedBrowsers(),
} = {}) {
  if (await checkBrowser()) {
    return false;
  }

  install();

  if (!(await checkBrowser())) {
    throw new Error(
      'Puppeteer still cannot locate Chrome after downloading browser artefacts.',
    );
  }

  return true;
}

/**
 * Runs the installer and records a failure in the process exit code.
 *
 * @param {() => boolean | Promise<boolean>} ensure - The install step.
 * @returns {Promise<void>} Resolves once the step has run or failed.
 */
export async function runCli(ensure = () => ensureBrowsersInstalled()) {
  try {
    await ensure();
  } catch (error) {
    console.error(error);
    process.exitCode = 1;
  }
}

if (isExecutedDirectly(import.meta)) {
  await runCli();
}