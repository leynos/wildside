/** @file Tests the Puppeteer cache priming used by the Mermaid renderer. */

import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  ensureBrowsersInstalled,
  hasLocalBrowser,
  installPinnedBrowsers,
  resolvePuppeteerCli,
  runCli,
} from './install-mermaid-browser.mjs';

const MISSING_CHROME = new Error('Could not find Chrome (ver. 123).');

afterEach(() => {
  vi.resetModules();
  vi.doUnmock('node:child_process');
  process.exitCode = undefined;
});

describe('hasLocalBrowser', () => {
  it('reports a browser whose executable exists', async () => {
    await expect(
      hasLocalBrowser({
        resolveExecutable: () => '/cache/chrome',
        isFile: () => true,
      }),
    ).resolves.toBe(true);
  });

  it('awaits the asynchronous Puppeteer 25 lookup', async () => {
    await expect(
      hasLocalBrowser({
        resolveExecutable: () => Promise.resolve('/cache/chrome'),
        isFile: () => true,
      }),
    ).resolves.toBe(true);
  });

  it('treats an empty executable path as missing', async () => {
    await expect(
      hasLocalBrowser({ resolveExecutable: () => '', isFile: () => true }),
    ).resolves.toBe(false);
  });

  it('treats Puppeteer reporting a missing Chrome as a cold cache', async () => {
    await expect(
      hasLocalBrowser({
        resolveExecutable: () => {
          throw MISSING_CHROME;
        },
      }),
    ).resolves.toBe(false);
  });

  it('rethrows an unrelated resolution failure', async () => {
    await expect(
      hasLocalBrowser({
        resolveExecutable: () => {
          throw new Error('permission denied');
        },
      }),
    ).rejects.toThrow('permission denied');
  });
});

describe('resolvePuppeteerCli', () => {
  it('resolves a string bin entry relative to the manifest', () => {
    expect(
      resolvePuppeteerCli({
        resolvePackageJson: () => '/repo/node_modules/puppeteer/package.json',
        readFile: () => JSON.stringify({ bin: './lib/puppeteer/node/cli.js' }),
      }),
    ).toBe('/repo/node_modules/puppeteer/lib/puppeteer/node/cli.js');
  });

  it('resolves the puppeteer key of a mapped bin entry', () => {
    expect(
      resolvePuppeteerCli({
        resolvePackageJson: () => '/repo/node_modules/puppeteer/package.json',
        readFile: () =>
          JSON.stringify({ bin: { puppeteer: 'lib/cli.js', other: 'x.js' } }),
      }),
    ).toBe('/repo/node_modules/puppeteer/lib/cli.js');
  });

  it('fails when the package publishes no bin entry', () => {
    expect(() =>
      resolvePuppeteerCli({
        resolvePackageJson: () => '/repo/node_modules/puppeteer/package.json',
        readFile: () => JSON.stringify({ name: 'puppeteer' }),
      }),
    ).toThrow('publishes no bin entry');
  });
});

describe('installPinnedBrowsers', () => {
  it('runs the pinned-browser install through the current Node binary', () => {
    const run = vi.fn(() => ({ status: 0, signal: null }));

    installPinnedBrowsers({
      cliPath: '/repo/node_modules/puppeteer/lib/cli.js',
      execPath: '/usr/bin/node',
      run,
    });

    expect(run).toHaveBeenCalledWith(
      '/usr/bin/node',
      ['/repo/node_modules/puppeteer/lib/cli.js', 'browsers', 'install'],
      { stdio: 'inherit' },
    );
  });

  it('fails when the install command exits non-zero', () => {
    expect(() =>
      installPinnedBrowsers({
        cliPath: '/repo/cli.js',
        run: () => ({ status: 2, signal: null }),
      }),
    ).toThrow('exited with status 2');
  });

  it('fails when the install command is signalled', () => {
    expect(() =>
      installPinnedBrowsers({
        cliPath: '/repo/cli.js',
        run: () => ({ status: null, signal: 'SIGTERM' }),
      }),
    ).toThrow('terminated by signal SIGTERM');
  });

  it('rethrows a spawn failure', () => {
    const error = new Error('spawn ENOENT');
    expect(() =>
      installPinnedBrowsers({
        cliPath: '/repo/cli.js',
        run: () => ({ error }),
      }),
    ).toThrow('spawn ENOENT');
  });
});

describe('ensureBrowsersInstalled', () => {
  it('downloads nothing when a browser is already present', async () => {
    const install = vi.fn();

    await expect(
      ensureBrowsersInstalled({ checkBrowser: () => true, install }),
    ).resolves.toBe(false);
    expect(install).not.toHaveBeenCalled();
  });

  it('installs once when the cache starts empty', async () => {
    const results = [false, true];
    const install = vi.fn();

    await expect(
      ensureBrowsersInstalled({
        checkBrowser: () => results.shift() ?? false,
        install,
      }),
    ).resolves.toBe(true);
    expect(install).toHaveBeenCalledTimes(1);
  });

  it('fails when the browser is still missing after installing', async () => {
    await expect(
      ensureBrowsersInstalled({
        checkBrowser: () => false,
        install: () => {},
      }),
    ).rejects.toThrow('still cannot locate Chrome');
  });
});

describe('runCli', () => {
  it('records a failed direct run in the exit code', async () => {
    await runCli(() => {
      throw new Error('install failed');
    });

    expect(process.exitCode).toBe(1);
  });

  it('leaves the exit code alone when the install succeeds', async () => {
    await runCli(() => true);

    expect(process.exitCode).toBeUndefined();
  });
});

describe('module import', () => {
  it('does not start an install as an import side effect', async () => {
    // The static import at the top of this file would otherwise leave the
    // module cached, so a run that selected only this test would exercise the
    // real `spawnSync` and pass without checking anything.
    vi.resetModules();
    const spawnSync = vi.fn(() => ({ status: 0, signal: null }));
    vi.doMock('node:child_process', () => ({ spawnSync }));

    await import('./install-mermaid-browser.mjs');

    expect(spawnSync).not.toHaveBeenCalled();
  });
});