/** @file Shared helpers for running dependency audits and reasoning about
 * advisories.
 *
 * These helpers centralise the JSON parsing and filtering logic used by the
 * security validation scripts. They keep the workspace wrappers aligned even
 * when the package manager needs a compatibility fallback.
 *
 * Cross-link: `frontend-pwa/scripts/run-audit.mjs` consumes these helpers to
 * enforce the validator patch requirement during workspace audits.
 */

import { execFileSync, spawnSync } from 'node:child_process';

const AUDIT_ARGS = ['audit', '--json'];
const LIST_ARGS = ['ls', '--json', '--depth', 'Infinity'];
const BULK_ADVISORY_PATH = '-/npm/v1/security/advisories/bulk';
const DEFAULT_REGISTRY = 'https://registry.npmjs.org/';
const COMMAND_MAX_BUFFER = 64 * 1024 * 1024;
const DEPENDENCY_SECTION_NAMES = [
  'dependencies',
  'devDependencies',
  'optionalDependencies',
];
const RETIRED_AUDIT_ENDPOINT_MESSAGE =
  'This endpoint is being retired. Use the bulk advisory endpoint instead.';

function parseJsonOutput(payloadText, commandLabel, options = {}) {
  const { requireNonEmpty = false } = options;
  const text = payloadText?.trim?.() ?? '';
  if (!text) {
    if (requireNonEmpty) {
      throw new Error(`Failed to parse ${commandLabel} JSON: response body was empty.`);
    }

    return {};
  }

  try {
    return JSON.parse(text);
  } catch (error) {
    error.message = `Failed to parse ${commandLabel} JSON: ${error.message}`;
    throw error;
  }
}

function isRetiredAuditEndpoint(payload) {
  return (
    payload?.error?.code === 'ERR_PNPM_AUDIT_BAD_RESPONSE' &&
    typeof payload?.error?.message === 'string' &&
    payload.error.message.includes(RETIRED_AUDIT_ENDPOINT_MESSAGE)
  );
}

function isLocalWorkspaceVersion(version) {
  return (
    version.startsWith('file:') ||
    version.startsWith('link:') ||
    version.startsWith('workspace:')
  );
}

function addPackageVersion(versionsByPackage, packageName, version) {
  const isMissing = !packageName || !version;
  if (isMissing || isLocalWorkspaceVersion(version)) {
    return;
  }

  const knownVersions = versionsByPackage.get(packageName) ?? new Set();
  knownVersions.add(version);
  versionsByPackage.set(packageName, knownVersions);
}

/**
 * Walk a dependency tree from `pnpm ls` and record the versions discovered.
 *
 * @param {Record<string, unknown> | undefined} node Dependency tree node from
 *   `pnpm ls`.
 * @param {Map<string, Set<string>>} versionsByPackage Collected versions keyed
 *   by package name.
 * @example
 * const versions = new Map();
 * walkDependencies({ dependencies: { validator: { version: '1.0.0' } } }, versions);
 * console.log([...versions.get('validator') ?? []]); // ['1.0.0']
 */
function walkDependencies(node, versionsByPackage) {
  if (!node || typeof node !== 'object') {
    return;
  }

  for (const sectionName of DEPENDENCY_SECTION_NAMES) {
    const section = node[sectionName];
    if (!section || typeof section !== 'object') {
      continue;
    }

    for (const [packageName, dependency] of Object.entries(section)) {
      if (!dependency || typeof dependency !== 'object') {
        continue;
      }

      if (typeof dependency.version === 'string') {
        addPackageVersion(versionsByPackage, packageName, dependency.version);
      }

      walkDependencies(dependency, versionsByPackage);
    }
  }
}

function collectInstalledPackageVersions() {
  const result = spawnSync('pnpm', LIST_ARGS, {
    encoding: 'utf8',
    maxBuffer: COMMAND_MAX_BUFFER,
    stdio: ['ignore', 'pipe', 'inherit'],
  });

  if (result.error) {
    throw result.error;
  }

  const status = result.status ?? 0;
  if (status !== 0) {
    throw new Error(`pnpm ls failed without producing a dependency tree (exit status ${status}).`);
  }

  const packageTrees = parseJsonOutput(result.stdout?.trim() ?? '', 'pnpm ls');
  const versionsByPackage = new Map();

  for (const tree of Array.isArray(packageTrees) ? packageTrees : [packageTrees]) {
    walkDependencies(tree, versionsByPackage);
  }

  return Object.fromEntries(
    [...versionsByPackage.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([packageName, versions]) => [packageName, [...versions].sort()]),
  );
}

function normaliseRegistryUrl(rawRegistry) {
  const trimmed = String(rawRegistry ?? '').trim();
  const registry =
    trimmed && trimmed !== 'undefined' && trimmed !== 'null' ? trimmed : DEFAULT_REGISTRY;
  return registry.endsWith('/') ? registry : `${registry}/`;
}

function readRegistryUrl() {
  const envRegistry = process.env.npm_config_registry ?? process.env.NPM_CONFIG_REGISTRY;
  if (envRegistry) {
    return normaliseRegistryUrl(envRegistry);
  }

  try {
    return normaliseRegistryUrl(
      execFileSync('pnpm', ['config', 'get', 'registry'], {
        encoding: 'utf8',
      }),
    );
  } catch {
    return DEFAULT_REGISTRY;
  }
}

function extractGithubAdvisoryId(advisoryUrl) {
  if (typeof advisoryUrl !== 'string') {
    return undefined;
  }

  const match = advisoryUrl.match(/GHSA-[0-9a-z]{4}-[0-9a-z]{4}-[0-9a-z]{4}/i);
  return match?.[0];
}

function deriveAdvisoryKey(packageName, advisory) {
  const githubAdvisoryId = extractGithubAdvisoryId(advisory?.url);
  const key = githubAdvisoryId ?? `${packageName}:${String(advisory?.id ?? 'unknown')}`;
  return { key, githubAdvisoryId };
}

/**
 * Merge advisories for a single package into the shared accumulator,
 * skipping entries whose key is already present.
 *
 * @param {string} packageName
 * @param {unknown[]} packageAdvisories Validated array of raw advisory objects.
 * @param {Record<string, unknown>} advisories Accumulator mutated in place.
 */
function addPackageAdvisories(packageName, packageAdvisories, advisories) {
  for (const advisory of packageAdvisories) {
    const { key, githubAdvisoryId } = deriveAdvisoryKey(packageName, advisory);

    if (Object.hasOwn(advisories, key)) {
      continue;
    }

    advisories[key] = {
      ...advisory,
      github_advisory_id: githubAdvisoryId,
      package_name: packageName,
    };
  }
}

function normalizeBulkAdvisories(bulkPayload) {
  const advisories = {};

  for (const [packageName, packageAdvisories] of Object.entries(bulkPayload ?? {})) {
    if (!Array.isArray(packageAdvisories)) {
      continue;
    }

    addPackageAdvisories(packageName, packageAdvisories, advisories);
  }

  return advisories;
}

async function runBulkAdvisoryAudit() {
  const registryUrl = readRegistryUrl();
  const endpoint = new URL(BULK_ADVISORY_PATH, registryUrl);
  const response = await fetch(endpoint, {
    method: 'POST',
    headers: {
      accept: 'application/json',
      'content-type': 'application/json',
    },
    body: JSON.stringify(collectInstalledPackageVersions()),
  });
  const responseText = await response.text();

  if (!response.ok) {
    throw new Error(
      `Bulk advisory audit failed (${response.status} ${response.statusText}) at ${endpoint}: ${responseText || '<empty>'}`,
    );
  }

  const bulkPayload = parseJsonOutput(responseText, 'bulk advisory audit', {
    requireNonEmpty: true,
  });
  const advisories = normalizeBulkAdvisories(bulkPayload);

  return {
    json: { advisories },
    status: Object.keys(advisories).length === 0 ? 0 : 1,
  };
}

/**
 * Run `pnpm audit --json` and return the parsed payload alongside the exit
 * status. Whitespace-only output is treated as an empty advisory list so that
 * callers can rely on deterministic results even when pnpm prints nothing.
 *
 * Newer npm registries now retire the legacy audit endpoints used by pnpm.
 * When that happens, the helper falls back to npm's supported bulk advisory
 * endpoint using the installed PNPM dependency tree.
 *
 * @returns {{
 *   json: { advisories?: Record<string, unknown> },
 *   status: number,
 * }} Parsed audit
 *   output and the pnpm exit status (defaults to zero when undefined).
 * @example
 * const { json, status } = await runAuditJson();
 * if (status !== 0) {
 *   throw new Error('pnpm audit failed');
 * }
 * console.log(Object.keys(json.advisories ?? {}));
 */
export async function runAuditJson() {
  const result = spawnSync('pnpm', AUDIT_ARGS, {
    encoding: 'utf8',
    maxBuffer: COMMAND_MAX_BUFFER,
    stdio: ['ignore', 'pipe', 'inherit'],
  });

  if (result.error) {
    throw result.error;
  }

  const status = result.status ?? 0;
  const stdout = result.stdout ? result.stdout.trim() : '';

  if (!stdout) {
    return { json: { advisories: {} }, status };
  }

  const json = parseJsonOutput(stdout, 'pnpm audit');
  if (isRetiredAuditEndpoint(json)) {
    return runBulkAdvisoryAudit();
  }

  return { json, status };
}

/**
 * Convert the advisories object returned by `pnpm audit` into a flat array that
 * is easier to filter.
 *
 * @param {{ advisories?: Record<string, unknown> }} auditJson Raw JSON payload
 *   from `pnpm audit`.
 * @returns {Array<Record<string, unknown>>} List of advisory objects.
 * @example
 * const advisories = collectAdvisories({ advisories: { "GHSA-123": { id: 1 } } });
 * console.log(advisories.length); // 1
 */
export function collectAdvisories(auditJson) {
  return Object.values(auditJson.advisories ?? {});
}

/**
 * Split advisories into those whose GitHub advisory IDs are present in the
 * allowed list and those that are unexpected.
 *
 * @param {Array<{ github_advisory_id?: string }>} advisories Advisories to
 *   partition.
 * @param {Iterable<string>} allowedIds Advisory IDs the caller expects.
 * @returns {{ expected: typeof advisories, unexpected: typeof advisories }}
 *   Partitioned advisories.
 * @example
 * const { expected, unexpected } = partitionAdvisoriesById(
 *   [
 *     { github_advisory_id: 'GHSA-1' },
 *     { github_advisory_id: 'GHSA-2' },
 *   ],
 *   ['GHSA-2'],
 * );
 * console.log(expected.length); // 1
 * console.log(unexpected.length); // 1
 */
export function partitionAdvisoriesById(advisories, allowedIds) {
  const allowed = new Set(allowedIds);
  const expected = [];
  const unexpected = [];

  for (const advisory of advisories) {
    const id = advisory.github_advisory_id;
    if (id && allowed.has(id)) {
      expected.push(advisory);
    } else {
      unexpected.push(advisory);
    }
  }

  return { expected, unexpected };
}

/**
 * Report unexpected advisories to stderr.
 *
 * @param {Array<{ github_advisory_id?: string, title?: string }>} unexpected
 *   Advisories that were not permitted.
 * @param {string} heading Descriptive heading for the error output.
 * @returns {boolean} Whether any advisories were reported.
 * @example
 * const hadUnexpected = reportUnexpectedAdvisories(
 *   [{ github_advisory_id: 'GHSA-1', title: 'Example' }],
 *   'Unexpected advisories:',
 * );
 * console.log(hadUnexpected); // true
 */
export function reportUnexpectedAdvisories(unexpected, heading) {
  if (unexpected.length === 0) {
    return false;
  }

  console.error(heading);
  for (const advisory of unexpected) {
    const id = advisory.github_advisory_id ?? 'UNKNOWN';
    const suffix = advisory.title ? `: ${advisory.title}` : '';
    console.error(`- ${id}${suffix}`);
  }
  return true;
}
