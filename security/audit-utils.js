/**
 * @file Shared helpers for dependency audits and advisory filtering.
 *
 * Provides the audit pipeline's JSON parsing, PNPM command execution, installed
 * package-version collection, bulk-advisory fallback, and advisory flattening
 * utilities. Pure helpers accept parsed JSON-shaped objects and return normalised
 * maps or arrays; effectful helpers cross the IO boundary through `auditIo`,
 * whose default implementation wraps filesystem, CLI, timer, and fetch effects.
 * `audit-reporting.js` owns advisory partitioning and stderr formatting, while
 * `validate-audit.js` applies policy to these normalised audit results. Callers
 * can assume exported helpers either return parsed audit data in the documented
 * shapes or throw explicit errors for failed, signalled, malformed, or
 * unavailable audit inputs.
 */

import { execFileSync, spawnSync } from 'node:child_process';
import {
  collectInstalledPackageVersions,
  normalizeBulkAdvisories,
  parseJsonOutput,
} from './audit-package-data.js';

export {
  buildVersionMap,
  collectInstalledPackageVersions,
  loadPackageTrees,
  normalizeBulkAdvisories,
  parseJsonOutput,
} from './audit-package-data.js';
export {
  partitionAdvisoriesById,
  reportUnexpectedAdvisories,
} from './audit-reporting.js';

const AUDIT_ARGS = ['audit', '--json'];
const BULK_ADVISORY_PATH = '-/npm/v1/security/advisories/bulk';
const BULK_AUDIT_TIMEOUT_MS = 30_000;
const DEFAULT_REGISTRY = 'https://registry.npmjs.org/';
const COMMAND_MAX_BUFFER = 64 * 1024 * 1024;
const RETIRED_AUDIT_ENDPOINT_MESSAGE = 'This endpoint is being retired. Use the bulk advisory endpoint instead.';
const defaultAuditIo = {
  execFileSync,
  fetch: (...args) => fetch(...args),
  setTimeout,
  clearTimeout,
  spawnSync,
};

/** Detect whether pnpm reported the retired audit endpoint.
 * @param {unknown} payload Parsed `pnpm audit --json` payload.
 * @returns {boolean} `true` when pnpm should fall back to the bulk advisory endpoint. @example isRetiredAuditEndpoint({ error: { code: 'ERR_PNPM_AUDIT_BAD_RESPONSE', message: 'Use the bulk advisory endpoint instead.' } }); // true
 */
function isRetiredAuditEndpoint(payload) {
  return (
    payload?.error?.code === 'ERR_PNPM_AUDIT_BAD_RESPONSE' &&
    typeof payload?.error?.message === 'string' &&
    payload.error.message.includes(RETIRED_AUDIT_ENDPOINT_MESSAGE));
}

/** Ensure a child-process result exited normally.
 * @param {{ error?: Error, signal?: string | null, status?: number | null }} result Spawn result from an audit command.
 * @param {string} commandLabel Label used in thrown errors.
 * @returns {number} Process exit status.
 * @example assertCompletedProcess({ status: 0, signal: null }, 'pnpm audit'); // 0
 */
function assertCompletedProcess(result, commandLabel) {
  if (result.error) {
    throw result.error;
  }
  if (result.signal) {
    throw new Error(`${commandLabel} was terminated by signal ${result.signal}.`);
  }
  if (result.status === null) {
    throw new Error(`${commandLabel} was terminated before reporting an exit status.`);
  }
  return result.status;
}

/** Return `true` when a raw registry string is a real URL and not a placeholder.
 * @param {string} value Trimmed registry string. @returns {boolean}
 * @example isValidRegistryValue('https://registry.npmjs.org'); // true
 */
function isValidRegistryValue(value) { return Boolean(value) && value !== 'undefined' && value !== 'null'; }

/** Normalize a registry URL so bulk advisory requests always target a valid base URL.
 * @param {string | undefined | null} rawRegistry Raw registry setting from env or pnpm config.
 * @returns {string} Registry URL with a trailing slash. @example normalizeRegistryUrl('https://registry.npmjs.org'); // 'https://registry.npmjs.org/'
 */
function normalizeRegistryUrl(rawRegistry) {
  const trimmed = String(rawRegistry ?? '').trim();
  const registry = isValidRegistryValue(trimmed) ? trimmed : DEFAULT_REGISTRY;
  return registry.endsWith('/') ? registry : `${registry}/`;
}

/** Read the npm registry URL from the environment or pnpm config.
 * @param {object} [auditIo=defaultAuditIo] Audit IO adapter; `defaultAuditIo` is used when omitted.
 * @returns {string} Normalised registry URL, or the npm default when lookup fails.
 * @example // With `npm_config_registry=https://registry.npmjs.org`: readRegistryUrl(); // 'https://registry.npmjs.org/'
 * @example const auditIo = { ...defaultAuditIo, execFileSync: () => 'https://registry.npmjs.org\n' }; readRegistryUrl(auditIo); // 'https://registry.npmjs.org/'
 */
function readRegistryUrl(auditIo = defaultAuditIo) {
  const envRegistry = process.env.npm_config_registry ?? process.env.NPM_CONFIG_REGISTRY;
  if (envRegistry) {
    return normalizeRegistryUrl(envRegistry);
  }
  try {
    return normalizeRegistryUrl(
      auditIo.execFileSync('pnpm', ['config', 'get', 'registry'], {
        encoding: 'utf8',
      }),
    );
  } catch {
    return DEFAULT_REGISTRY;
  }
}

/** Post package versions to the npm bulk advisory endpoint and return the raw response.
 * @param {URL} endpoint Bulk advisory endpoint URL. @param {Record<string, string[]>} packageVersions Installed package versions keyed by package name.
 * @param {object} [auditIo=defaultAuditIo] Audit IO adapter; `defaultAuditIo` is used when omitted.
 * @returns {Promise<{ response: Response, responseText: string }>} HTTP response and response body text.
 * @example const { responseText } = await fetchBulkAdvisories(new URL('https://registry.npmjs.org/-/npm/v1/security/advisories/bulk'), { validator: ['13.15.23'] }); console.log(responseText); // '{}'
 * @example const auditIo = { ...defaultAuditIo, fetch: async () => ({ text: async () => '{}' }), setTimeout: () => 1, clearTimeout: () => undefined }; await fetchBulkAdvisories(new URL('https://registry.npmjs.org/-/npm/v1/security/advisories/bulk'), {}, auditIo); // { response: ..., responseText: '{}' }
 */
async function fetchBulkAdvisories(endpoint, packageVersions, auditIo = defaultAuditIo) {
  const controller = new AbortController();
  const timeoutId = auditIo.setTimeout(() => controller.abort(), BULK_AUDIT_TIMEOUT_MS);
  try {
    const response = await auditIo.fetch(endpoint, {
      method: 'POST',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
      },
      body: JSON.stringify(packageVersions),
      signal: controller.signal,
    });
    const responseText = await response.text();
    return { response, responseText };
  } catch (error) {
    if (error?.name === 'AbortError') {
      throw new Error(`Bulk advisory audit timed out after ${BULK_AUDIT_TIMEOUT_MS}ms at ${endpoint}`);
    }
    throw error;
  } finally {
    auditIo.clearTimeout(timeoutId);
  }
}

/** Convert normalized advisories into the shared audit result structure.
 * @param {Record<string, unknown>} advisories Normalized advisories keyed by advisory identifier.
 * @returns {{ json: { advisories: Record<string, unknown> }, status: number }} Audit result payload and exit status. @example toAdvisoryResult({}); // { json: { advisories: {} }, status: 0 }
 */
function toAdvisoryResult(advisories) {
  return { json: { advisories }, status: Object.keys(advisories).length === 0 ? 0 : 1 };
}

/** Query the npm bulk advisory endpoint using the installed PNPM dependency tree.
 * @param {object} [auditIo=defaultAuditIo] Audit IO adapter; `defaultAuditIo` is used when omitted.
 * @returns {Promise<{ json: { advisories: Record<string, unknown> }, status: number }>} Bulk advisory payload and derived exit status.
 * @example // With a successful bulk advisory response containing one advisory:
 * await runBulkAdvisoryAudit(); // { json: { advisories: { 'GHSA-vghf-hv5q-vc2g': { ... } } }, status: 1 }
 * @example const auditIo = { ...defaultAuditIo, spawnSync: () => ({ status: 0, stdout: '[{"dependencies":{}}]' }), fetch: async () => ({ ok: true, text: async () => '{}' }) }; await runBulkAdvisoryAudit(auditIo); // { json: { advisories: {} }, status: 0 }
 */
async function runBulkAdvisoryAudit(auditIo = defaultAuditIo) {
  const registryUrl = readRegistryUrl(auditIo);
  const endpoint = new URL(BULK_ADVISORY_PATH, registryUrl);
  const { response, responseText } = await fetchBulkAdvisories(
    endpoint,
    collectInstalledPackageVersions(auditIo, assertCompletedProcess),
    auditIo,
  );

  if (!response.ok) {
    throw new Error(
      `Bulk advisory audit failed (${response.status} ${response.statusText}) at ${endpoint}: ${responseText || '<empty>'}`,
    );
  }

  const bulkPayload = parseJsonOutput(responseText, 'bulk advisory audit', { requireNonEmpty: true });

  return toAdvisoryResult(normalizeBulkAdvisories(bulkPayload));
}

/** Run `pnpm audit --json`, falling back to the bulk advisory endpoint when needed.
 * Throws when `pnpm audit` fails to start or is signalled.
 * @param {object} [auditIo=defaultAuditIo] Audit IO adapter; `defaultAuditIo` is used when omitted.
 * @returns {Promise<{ json: { advisories?: Record<string, unknown> }, status: number }>} Parsed audit output and pnpm exit status.
 * @example const { json, status } = await runAuditJson(); console.log(status, Object.keys(json.advisories ?? {}));
 * @example const auditIo = { ...defaultAuditIo, spawnSync: () => ({ status: 0, stdout: '{"advisories":{}}' }) }; await runAuditJson(auditIo); // { json: { advisories: {} }, status: 0 }
 */
export async function runAuditJson(auditIo = defaultAuditIo) {
  const result = auditIo.spawnSync('pnpm', AUDIT_ARGS, {
    encoding: 'utf8',
    maxBuffer: COMMAND_MAX_BUFFER,
    stdio: ['ignore', 'pipe', 'inherit'],
  });
  const status = assertCompletedProcess(result, 'pnpm audit');
  const stdout = result.stdout ? result.stdout.trim() : '';
  if (!stdout) {
    return { json: { advisories: {} }, status };
  }

  const json = parseJsonOutput(stdout, 'pnpm audit');
  if (isRetiredAuditEndpoint(json)) {
    return runBulkAdvisoryAudit(auditIo);
  }

  return { json, status };
}

/** Convert the advisories object returned by `pnpm audit` into a flat array.
 * @param {{ advisories?: Record<string, unknown> }} auditJson Raw JSON payload from `pnpm audit`.
 * @returns {Array<Record<string, unknown>>} List of advisory objects. @example const advisories = collectAdvisories({ advisories: { "GHSA-123": { id: 1 } } }); console.log(advisories.length); // 1
 */
export function collectAdvisories(auditJson) {
  return Object.values(auditJson.advisories ?? {});
}
