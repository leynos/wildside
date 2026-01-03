/**
 * @file Ensures the Chrome binaries required by Puppeteer are available before
 * Mermaid diagrams are rendered in CI.
 *
 * Nixie shells out to `mmdc`, which depends on Puppeteer finding the
 * `chrome-headless-shell` executable. The GitHub runners used in CI start with
 * an empty Puppeteer cache, so we proactively download the browser artefacts
 * that match the version bundled with our dependencies. This keeps the `make
 * nixie` target reproducible without relying on a manual pre-install step.
 */

import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import * as puppeteer from 'puppeteer';
import {
  Browser,
  detectBrowserPlatform,
  install,
  resolveBuildId,
} from '@puppeteer/browsers';

const MISSING_BROWSER_MESSAGE_FRAGMENT = 'Could not find Chrome';
const DEFAULT_CACHE_DIR = join(homedir(), '.cache', 'puppeteer');

async function resolveBrowserVersion() {
  if (typeof puppeteer.browserVersion === 'function') {
    return await puppeteer.browserVersion();
  }

  if (typeof puppeteer.defaultBrowserRevision === 'string') {
    return puppeteer.defaultBrowserRevision;
  }

  if (puppeteer.PUPPETEER_REVISIONS?.chrome) {
    return puppeteer.PUPPETEER_REVISIONS.chrome;
  }

  if (puppeteer.PUPPETEER_REVISIONS?.chromium) {
    return puppeteer.PUPPETEER_REVISIONS.chromium;
  }

  throw new Error('Unable to resolve the Puppeteer browser version.');
}

function resolveBrowserType() {
  if (puppeteer.PUPPETEER_REVISIONS?.chrome) {
    return Browser.CHROME;
  }

  return Browser.CHROMIUM;
}

function hasLocalBrowser() {
  try {
    const browserPath = puppeteer.executablePath();
    return Boolean(browserPath) && existsSync(browserPath);
  } catch (error) {
    if (error instanceof Error && error.message.includes(MISSING_BROWSER_MESSAGE_FRAGMENT)) {
      return false;
    }

    throw error;
  }
}

async function ensureBrowsersInstalled() {
  if (hasLocalBrowser()) {
    return;
  }

  const platform = detectBrowserPlatform();

  if (!platform) {
    throw new Error('Puppeteer does not support the current platform.');
  }

  const browser = resolveBrowserType();
  const browserVersion = await resolveBrowserVersion();
  const buildId = await resolveBuildId(browser, platform, browserVersion);

  await install({
    browser,
    buildId,
    cacheDir: process.env.PUPPETEER_CACHE_DIR ?? DEFAULT_CACHE_DIR,
  });

  if (!hasLocalBrowser()) {
    throw new Error('Puppeteer still cannot locate Chrome after downloading browser artefacts.');
  }
}

export async function main() {
  await ensureBrowsersInstalled();
}

const executedScriptUrl =
  process.argv[1] === undefined ? undefined : pathToFileURL(process.argv[1]).href;

if (executedScriptUrl === import.meta.url) {
  // Preserve the original side effect for the `make nixie` workflow while
  // allowing other modules to import `main` without triggering a download.
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
