#!/usr/bin/env node

/**
 * Verifies that security-sensitive dependency overrides stay scoped to pnpm.
 *
 * npm consumes top-level `overrides` for every command, including `npx`, and
 * rejects overrides that conflict with direct dependency ranges. The workspace
 * therefore keeps install-time dependency patches under `pnpm.overrides` only.
 */

import { readFile } from 'node:fs/promises';

import { isExecutedDirectly } from './direct-execution.mjs';

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

function isRecord(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

/**
 * Builds a pure report for the workspace override policy.
 *
 * @param {Record<string, unknown>} packageJson - The parsed package manifest.
 * @returns {{
 *   ok: boolean,
 *   pnpmOverridesToCheck: string[],
 *   rootOverrides: string[],
 *   reason: 'matched' | 'missing-pnpm-overrides' | 'root-overrides-present',
 * }} Structured policy result.
 */
export function checkOverridesPolicy(packageJson) {
  // npm consumes any top-level `overrides` property, so reject its mere
  // presence — empty object, populated, or malformed (non-object) — rather than
  // only a non-empty one.
  const hasRootOverrides = Object.hasOwn(packageJson, 'overrides');
  const rootOverrides = isRecord(packageJson.overrides)
    ? Object.keys(packageJson.overrides).sort()
    : [];
  const pnpmOverrides = isRecord(packageJson.pnpm?.overrides)
    ? Object.keys(packageJson.pnpm.overrides).sort()
    : [];

  if (hasRootOverrides) {
    return {
      ok: false,
      pnpmOverridesToCheck: pnpmOverrides,
      rootOverrides,
      reason: 'root-overrides-present',
    };
  }

  if (pnpmOverrides.length === 0) {
    return {
      ok: false,
      pnpmOverridesToCheck: pnpmOverrides,
      rootOverrides,
      reason: 'missing-pnpm-overrides',
    };
  }

  return {
    ok: true,
    pnpmOverridesToCheck: pnpmOverrides,
    rootOverrides,
    reason: 'matched',
  };
}

/**
 * Writes an override policy report to the supplied console-like adapter.
 *
 * @param {ReturnType<typeof checkOverridesPolicy>} report - The structured report.
 * @param {{log: (...args: unknown[]) => void, error: (...args: unknown[]) => void}} outputIo - Output adapter.
 * @returns {number} `0` when policy passes, otherwise `1`.
 */
export function reportOverridesPolicy(report, outputIo = console) {
  if (report.ok) {
    outputIo.log(
      `pnpm override policy verified for ${report.pnpmOverridesToCheck.join(', ')}.`,
    );
    return 0;
  }

  if (report.reason === 'missing-pnpm-overrides') {
    outputIo.error('Override policy check failed.\nNo pnpm.overrides entries were found.');
    return 1;
  }

  outputIo.error(
    [
      'Override policy check failed.',
      'Top-level overrides are not allowed because npm and npx consume them.',
      `Move these entries under pnpm.overrides only: ${report.rootOverrides.join(', ')}`,
    ].join('\n'),
  );
  return 1;
}

if (isExecutedDirectly(import.meta)) {
  try {
    const packageJson = await readPackageJson();
    process.exitCode = reportOverridesPolicy(checkOverridesPolicy(packageJson));
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
