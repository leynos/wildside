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
import { executablePath } from 'puppeteer';
import { downloadBrowsers } from 'puppeteer/lib/esm/puppeteer/node/install.js';

const MISSING_BROWSER_MESSAGE_FRAGMENT = 'Could not find Chrome';

function hasLocalBrowser() {
  try {
    const browserPath = executablePath();
    return Boolean(browserPath) && existsSync(browserPath);
  } catch (error) {
    if (
      error instanceof Error &&
      typeof error.message === 'string' &&
      error.message.includes(MISSING_BROWSER_MESSAGE_FRAGMENT)
    ) {
      return false;
    }

    throw error;
  }
}

async function ensureBrowsersInstalled() {
  if (hasLocalBrowser()) {
    return;
  }

  await downloadBrowsers();

  if (!hasLocalBrowser()) {
    throw new Error(
      'Puppeteer still cannot locate Chrome after downloading browser artefacts.'
    );
  }
}

await ensureBrowsersInstalled();
