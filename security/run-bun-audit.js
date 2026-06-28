#!/usr/bin/env node

/** @file Run `bun audit` with ignores sourced from the audit exception ledger. */

import { spawnSync } from 'node:child_process';
import { realpathSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import auditExceptions from './audit-exceptions.json' with { type: 'json' };
import { assertNoExpired } from './audit-exception-policy.js';

const normalise = (path) =>
  typeof realpathSync.native === 'function' ? realpathSync.native(path) : realpathSync(path);

/**
 * Build the `bun audit` command arguments for the supplied exception ledger.
 *
 * @param {Array<{advisory: string}>} entries Audit exception entries.
 * @returns {string[]} Arguments for `bun`.
 * @example
 * buildBunAuditArgs([{ advisory: 'GHSA-vghf-hv5q-vc2g' }]);
 */
export function buildBunAuditArgs(entries) {
  const advisoryIds = [...new Set(entries.map((entry) => entry.advisory))].sort();
  return ['audit', ...advisoryIds.map((id) => `--ignore=${id}`)];
}

/**
 * Run Bun's audit command with ledger-backed advisory ignores.
 *
 * @param {Array<{advisory: string}>} entries Audit exception entries.
 * @param {{spawnSync: typeof spawnSync}} [auditIo] Process adapter.
 * @returns {number} Process exit status.
 * @example
 * runBunAudit([], { spawnSync: () => ({ status: 0, signal: null }) });
 */
export function runBunAudit(entries, auditIo = { spawnSync }) {
  assertNoExpired(entries);
  const result = auditIo.spawnSync('bun', buildBunAuditArgs(entries), {
    stdio: 'inherit',
  });

  if (result.error) {
    throw result.error;
  }

  if (result.signal) {
    throw new Error(`bun audit was terminated by signal ${result.signal}.`);
  }

  return result.status ?? 1;
}

function isExecutedDirectly(meta) {
  const invokedPath = process.argv?.[1];
  if (!invokedPath) {
    return false;
  }

  try {
    const scriptPath = fileURLToPath(meta.url);
    return normalise(scriptPath) === normalise(resolve(invokedPath));
  } catch {
    return false;
  }
}

if (isExecutedDirectly(import.meta)) {
  try {
    process.exitCode = runBunAudit(auditExceptions);
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
