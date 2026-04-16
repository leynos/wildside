/** @file Shared helpers for dependency audits and advisory filtering. */

import { execFileSync, spawnSync } from 'node:child_process';

const AUDIT_ARGS = ['audit', '--json'];
const LIST_ARGS = ['ls', '--json', '--depth', 'Infinity'];
const BULK_ADVISORY_PATH = '-/npm/v1/security/advisories/bulk';
const BULK_AUDIT_TIMEOUT_MS = 30_000;
const DEFAULT_REGISTRY = 'https://registry.npmjs.org/';
const COMMAND_MAX_BUFFER = 64 * 1024 * 1024;
const DEPENDENCY_SECTION_NAMES = [
  'dependencies',
  'devDependencies',
  'optionalDependencies',
];
const RETIRED_AUDIT_ENDPOINT_MESSAGE =
  'This endpoint is being retired. Use the bulk advisory endpoint instead.';

/** Parse command JSON and optionally reject blank responses.
 * @param {string | undefined | null} payloadText Raw command output. @param {string} commandLabel Label used in parse errors. @param {{ requireNonEmpty?: boolean }} [options={}] Parsing options.
 * @returns {Record<string, unknown> | unknown[]} Parsed JSON value, or `{}` for optional blank output. @example parseJsonOutput('{"advisories":{}}', 'pnpm audit'); // { advisories: {} }
 */
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

/** Detect whether pnpm reported the retired audit endpoint.
 * @param {unknown} payload Parsed `pnpm audit --json` payload.
 * @returns {boolean} `true` when pnpm should fall back to the bulk advisory endpoint. @example isRetiredAuditEndpoint({ error: { code: 'ERR_PNPM_AUDIT_BAD_RESPONSE', message: 'Use the bulk advisory endpoint instead.' } }); // true
 */
function isRetiredAuditEndpoint(payload) {
  return (
    payload?.error?.code === 'ERR_PNPM_AUDIT_BAD_RESPONSE' &&
    typeof payload?.error?.message === 'string' &&
    payload.error.message.includes(RETIRED_AUDIT_ENDPOINT_MESSAGE)
  );
}

/** Check whether a version points at a local workspace dependency.
 * @param {string} version Package version or workspace reference.
 * @returns {boolean} `true` when the version should be ignored for registry audits. @example isLocalWorkspaceVersion('workspace:*'); // true
 */
function isLocalWorkspaceVersion(version) {
  return (
    version.startsWith('file:') ||
    version.startsWith('link:') ||
    version.startsWith('workspace:')
  );
}

/** Record an installed package version unless it is missing or workspace-local.
 * @param {Map<string, Set<string>>} versionsByPackage Installed versions keyed by package name. @param {string} packageName Package name from `pnpm ls`. @param {string} version Installed package version.
 * @returns {void} @example const versions = new Map(); addPackageVersion(versions, 'validator', '13.15.23'); console.log([...versions.get('validator')]); // ['13.15.23']
 */
function addPackageVersion(versionsByPackage, packageName, version) {
  const isMissing = !packageName || !version;
  if (isMissing || isLocalWorkspaceVersion(version)) {
    return;
  }

  const knownVersions = versionsByPackage.get(packageName) ?? new Set();
  knownVersions.add(version);
  versionsByPackage.set(packageName, knownVersions);
}

/** Walk one dependency section from `pnpm ls` and record installed versions.
 * @param {Record<string, unknown> | undefined} section Dependency section keyed by package name. @param {Map<string, Set<string>>} versionsByPackage Collected versions keyed by package name.
 */
function walkDependencySection(section, versionsByPackage) {
  if (!section || typeof section !== 'object') {
    return;
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

/** Walk a `pnpm ls` tree and record every installed package version.
 * @param {Record<string, unknown> | undefined} node Dependency tree node from `pnpm ls`. @param {Map<string, Set<string>>} versionsByPackage Collected versions keyed by package name.
 * @returns {void} @example const versions = new Map(); walkDependencies({ dependencies: { validator: { version: '13.15.23' } } }, versions); console.log([...versions.get('validator')]); // ['13.15.23']
 */
function walkDependencies(node, versionsByPackage) {
  if (!node || typeof node !== 'object') {
    return;
  }

  for (const sectionName of DEPENDENCY_SECTION_NAMES) {
    walkDependencySection(node[sectionName], versionsByPackage);
  }
}

/** Build the installed package-version map from parsed `pnpm ls` output.
 * @param {Record<string, unknown> | Record<string, unknown>[] | undefined} packageTrees Parsed `pnpm ls` output as one tree or many. @returns {Map<string, Set<string>>} Installed versions keyed by package name.
 */
function buildVersionMap(packageTrees) {
  const versionsByPackage = new Map();

  for (const tree of Array.isArray(packageTrees) ? packageTrees : [packageTrees]) {
    walkDependencies(tree, versionsByPackage);
  }

  return versionsByPackage;
}

/** Collect installed package versions from `pnpm ls` for bulk advisory lookups.
 * @returns {Record<string, string[]>} Sorted installed versions keyed by package name. @example // With `pnpm ls` returning one installed validator version: collectInstalledPackageVersions(); // { validator: ['13.15.23'] }
 */
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
  const versionsByPackage = buildVersionMap(packageTrees);

  return Object.fromEntries(
    [...versionsByPackage.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([packageName, versions]) => [packageName, [...versions].sort()]),
  );
}

/** Normalize a registry URL so bulk advisory requests always target a valid base URL.
 * @param {string | undefined | null} rawRegistry Raw registry setting from env or pnpm config.
 * @returns {string} Registry URL with a trailing slash. @example normalizeRegistryUrl('https://registry.npmjs.org'); // 'https://registry.npmjs.org/'
 */
function normalizeRegistryUrl(rawRegistry) {
  const trimmed = String(rawRegistry ?? '').trim();
  const registry =
    trimmed && trimmed !== 'undefined' && trimmed !== 'null' ? trimmed : DEFAULT_REGISTRY;
  return registry.endsWith('/') ? registry : `${registry}/`;
}

/** Read the npm registry URL from the environment or pnpm config.
 * @returns {string} Normalised registry URL, or the npm default when lookup fails. @example // With `npm_config_registry=https://registry.npmjs.org`: readRegistryUrl(); // 'https://registry.npmjs.org/'
 */
function readRegistryUrl() {
  const envRegistry = process.env.npm_config_registry ?? process.env.NPM_CONFIG_REGISTRY;
  if (envRegistry) {
    return normalizeRegistryUrl(envRegistry);
  }

  try {
    return normalizeRegistryUrl(
      execFileSync('pnpm', ['config', 'get', 'registry'], {
        encoding: 'utf8',
      }),
    );
  } catch {
    return DEFAULT_REGISTRY;
  }
}

/** Extract a GitHub advisory identifier from an advisory URL.
 * @param {unknown} advisoryUrl Advisory URL from pnpm or npm audit output.
 * @returns {string | undefined} Matching GHSA identifier when one is present. @example extractGithubAdvisoryId('https://github.com/advisories/GHSA-vghf-hv5q-vc2g'); // 'GHSA-vghf-hv5q-vc2g'
 */
function extractGithubAdvisoryId(advisoryUrl) {
  if (typeof advisoryUrl !== 'string') {
    return undefined;
  }

  const match = advisoryUrl.match(/GHSA-[0-9a-z]{4}-[0-9a-z]{4}-[0-9a-z]{4}/i);
  return match?.[0];
}

/** Derive the advisory key used to deduplicate bulk advisory responses.
 * @param {string} packageName Advisory package name. @param {{ id?: unknown, url?: unknown }} advisory Raw advisory object.
 * @returns {{ key: string, githubAdvisoryId: string | undefined }} Stable advisory key and extracted GHSA identifier. @example deriveAdvisoryKey('validator', { id: 100000, url: 'https://github.com/advisories/GHSA-vghf-hv5q-vc2g' }); // { key: 'GHSA-vghf-hv5q-vc2g', githubAdvisoryId: 'GHSA-vghf-hv5q-vc2g' }
 */
function deriveAdvisoryKey(packageName, advisory) {
  const githubAdvisoryId = extractGithubAdvisoryId(advisory?.url);
  const key = githubAdvisoryId ?? `${packageName}:${String(advisory?.id ?? 'unknown')}`;
  return { key, githubAdvisoryId };
}

/** Merge advisories for one package into the shared accumulator.
 * @param {string} packageName Package name from the bulk advisory payload. @param {unknown[]} packageAdvisories Validated array of raw advisory objects. @param {Record<string, unknown>} advisories Accumulator mutated in place.
 * @returns {void} @example const advisories = {}; addPackageAdvisories('validator', [{ id: 100000, url: 'https://github.com/advisories/GHSA-vghf-hv5q-vc2g', title: 'Validator SSRF' }], advisories); console.log(advisories['GHSA-vghf-hv5q-vc2g'].package_name); // 'validator'
 */
function addPackageAdvisories(packageName, packageAdvisories, advisories) {
  for (const [index, advisory] of packageAdvisories.entries()) {
    const isPlainObject =
      typeof advisory === 'object' && advisory !== null && !Array.isArray(advisory);
    if (!isPlainObject) {
      throw new Error(`Invalid advisory for package ${packageName} at index ${index}: expected object`);
    }

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

/** Normalize bulk advisory responses into the shared advisory object shape.
 * @param {Record<string, unknown> | undefined} bulkPayload Bulk advisory payload keyed by package name.
 * @returns {Record<string, unknown>} Deduplicated advisories keyed by GHSA identifier or package fallback. @example normalizeBulkAdvisories({ validator: [{ id: 100000, url: 'https://github.com/advisories/GHSA-vghf-hv5q-vc2g' }] }); // { 'GHSA-vghf-hv5q-vc2g': { github_advisory_id: 'GHSA-vghf-hv5q-vc2g', package_name: 'validator', id: 100000, url: 'https://github.com/advisories/GHSA-vghf-hv5q-vc2g' } }
 */
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

/** Post package versions to the npm bulk advisory endpoint and return the raw response.
 * @param {URL} endpoint Bulk advisory endpoint URL. @param {Record<string, string[]>} packageVersions Installed package versions keyed by package name.
 * @returns {Promise<{ response: Response, responseText: string }>} HTTP response and response body text.
 */
async function fetchBulkAdvisories(endpoint, packageVersions) {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), BULK_AUDIT_TIMEOUT_MS);

  try {
    const response = await fetch(endpoint, {
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
    clearTimeout(timeoutId);
  }
}

/** Convert normalized advisories into the shared audit result structure.
 * @param {Record<string, unknown>} advisories Normalized advisories keyed by advisory identifier.
 * @returns {{ json: { advisories: Record<string, unknown> }, status: number }} Audit result payload and exit status.
 */
function toAdvisoryResult(advisories) {
  return { json: { advisories }, status: Object.keys(advisories).length === 0 ? 0 : 1 };
}

/** Query the npm bulk advisory endpoint using the installed PNPM dependency tree.
 * @returns {Promise<{ json: { advisories: Record<string, unknown> }, status: number }>} Bulk advisory payload and derived exit status. @example // With a successful bulk advisory response containing one advisory: await runBulkAdvisoryAudit(); // { json: { advisories: { 'GHSA-vghf-hv5q-vc2g': { ... } } }, status: 1 }
 */
async function runBulkAdvisoryAudit() {
  const registryUrl = readRegistryUrl();
  const endpoint = new URL(BULK_ADVISORY_PATH, registryUrl);
  const { response, responseText } = await fetchBulkAdvisories(endpoint, collectInstalledPackageVersions());

  if (!response.ok) {
    throw new Error(
      `Bulk advisory audit failed (${response.status} ${response.statusText}) at ${endpoint}: ${responseText || '<empty>'}`,
    );
  }

  const bulkPayload = parseJsonOutput(responseText, 'bulk advisory audit', { requireNonEmpty: true });

  return toAdvisoryResult(normalizeBulkAdvisories(bulkPayload));
}

/** Run `pnpm audit --json`, falling back to the bulk advisory endpoint when needed.
 * @returns {{ json: { advisories?: Record<string, unknown> }, status: number }} Parsed audit output and pnpm exit status. @example const { json, status } = await runAuditJson(); console.log(status, Object.keys(json.advisories ?? {}));
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

/** Convert the advisories object returned by `pnpm audit` into a flat array.
 * @param {{ advisories?: Record<string, unknown> }} auditJson Raw JSON payload from `pnpm audit`.
 * @returns {Array<Record<string, unknown>>} List of advisory objects. @example const advisories = collectAdvisories({ advisories: { "GHSA-123": { id: 1 } } }); console.log(advisories.length); // 1
 */
export function collectAdvisories(auditJson) {
  return Object.values(auditJson.advisories ?? {});
}

/** Split advisories into allowed and unexpected groups.
 * @param {Array<{ github_advisory_id?: string }>} advisories Advisories to partition. @param {Iterable<string>} allowedIds Advisory IDs the caller expects.
 * @returns {{ expected: typeof advisories, unexpected: typeof advisories }} Partitioned advisories.
 * @example const { expected, unexpected } = partitionAdvisoriesById([{ github_advisory_id: 'GHSA-1' }, { github_advisory_id: 'GHSA-2' }], ['GHSA-2']); console.log(expected.length); // 1
 * @example console.log(unexpected.length); // 1
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

/** Format one advisory as a report line.
 * @param {{ github_advisory_id?: string, title?: string }} advisory Advisory to print. @returns {string} Human-readable bullet line for the advisory.
 */
function formatAdvisoryLine(advisory) {
  const id = advisory.github_advisory_id ?? 'UNKNOWN';
  const suffix = advisory.title ? `: ${advisory.title}` : '';
  return `- ${id}${suffix}`;
}

/** Report unexpected advisories to stderr.
 * @param {Array<{ github_advisory_id?: string, title?: string }>} unexpected Advisories that were not permitted. @param {string} heading Descriptive heading for the error output.
 * @returns {boolean} Whether any advisories were reported. @example const hadUnexpected = reportUnexpectedAdvisories([{ github_advisory_id: 'GHSA-1', title: 'Example' }], 'Unexpected advisories:'); console.log(hadUnexpected); // true
 */
export function reportUnexpectedAdvisories(unexpected, heading) {
  if (unexpected.length === 0) {
    return false;
  }

  console.error(heading);
  for (const advisory of unexpected) {
    console.error(formatAdvisoryLine(advisory));
  }
  return true;
}
