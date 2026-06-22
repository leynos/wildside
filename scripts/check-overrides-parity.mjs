#!/usr/bin/env node

/**
 * Verifies that Bun and pnpm override entries stay aligned for security fixes
 * that must resolve consistently across both install paths.
 */

import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const PACKAGE_JSON_PATH = new URL('../package.json', import.meta.url);

/**
 * Formats an override value for human-readable diagnostics.
 *
 * @param {unknown} value - The override value to display.
 * @returns {string} A stable string representation for logs.
 */
export function formatOverrideValue(value) {
  return value === undefined ? '<missing>' : JSON.stringify(value);
}

/**
 * Reads and parses the root package manifest.
 *
 * @returns {Promise<Record<string, unknown>>} The parsed package manifest.
 */
async function readPackageJson() {
  const source = await readFile(PACKAGE_JSON_PATH, 'utf8');
  return JSON.parse(source);
}

/**
 * Reports drift between root overrides and pnpm overrides.
 *
 * @param {Record<string, unknown>} packageJson - The parsed package manifest.
 * @returns {number} `0` when overrides match, otherwise `1`.
 */
export function checkOverridesParity(packageJson) {
  const rootOverrides = packageJson.overrides ?? {};
  const pnpmOverrides = packageJson.pnpm?.overrides ?? {};
  const overridesToCheck = [
    ...new Set([...Object.keys(rootOverrides), ...Object.keys(pnpmOverrides)]),
  ].sort();

  if (overridesToCheck.length === 0) {
    console.error(
      [
        'Override parity check failed.',
        'No overrides were found in overrides or pnpm.overrides.',
      ].join('\n'),
    );
    return 1;
  }

  const mismatches = overridesToCheck.flatMap((dependencyName) => {
    const rootValue = rootOverrides[dependencyName];
    const pnpmValue = pnpmOverrides[dependencyName];

    if (rootValue === pnpmValue && rootValue !== undefined) {
      return [];
    }

    return [
      [
        `Override mismatch for "${dependencyName}":`,
        `  overrides: ${formatOverrideValue(rootValue)}`,
        `  pnpm.overrides: ${formatOverrideValue(pnpmValue)}`,
      ].join('\n'),
    ];
  });

  if (mismatches.length === 0) {
    console.log(
      `Override parity verified for ${overridesToCheck.join(', ')}.`,
    );
    return 0;
  }

  console.error(
    [
      'Override parity check failed.',
      ...mismatches,
    ].join('\n'),
  );
  return 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const packageJson = await readPackageJson();
  process.exitCode = checkOverridesParity(packageJson);
}
