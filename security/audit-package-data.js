/**
 * @file Package-tree and advisory-normalisation helpers for audit utilities.
 *
 * Owns `pnpm ls` serialisation, installed-version map construction, and npm
 * bulk-advisory response normalisation. Callers provide parsed JSON-shaped
 * objects for pure helpers or an `auditIo` adapter for command execution.
 */

const LIST_ARGS = ['ls', '--json', '--depth', 'Infinity'];
const COMMAND_MAX_BUFFER = 64 * 1024 * 1024;
const DEPENDENCY_SECTION_NAMES = ['dependencies', 'devDependencies', 'optionalDependencies'];

/** Parse command JSON and optionally reject blank responses.
 * @param {string | undefined | null} payloadText Raw command output. @param {string} commandLabel Label used in parse errors. @param {{ requireNonEmpty?: boolean }} [options={}] Parsing options.
 * @returns {Record<string, unknown> | unknown[]} Parsed JSON value, or `{}` for optional blank output. @example parseJsonOutput('{"advisories":{}}', 'pnpm audit'); // { advisories: {} }
 */
export function parseJsonOutput(payloadText, commandLabel, options = {}) {
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

/** Check whether a version points at a local workspace dependency.
 * @param {string} version Package version or workspace reference.
 * @returns {boolean} `true` when the version should be ignored for registry audits. @example isLocalWorkspaceVersion('workspace:*'); // true
 */
function isLocalWorkspaceVersion(version) {
  return (
    version.startsWith('file:') ||
    version.startsWith('link:') ||
    version.startsWith('workspace:'));
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

/** Process a single entry from a pnpm ls dependency section.
 * @param {string} packageName Package name.
 * @param {unknown} dependency Raw dependency value from the section.
 * @param {Map<string, Set<string>>} versionsByPackage Collected versions keyed by package name.
 * @returns {void}
 */
function processDependencyEntry(packageName, dependency, versionsByPackage) {
  if (!dependency || typeof dependency !== 'object') {
    return;
  }
  if (typeof dependency.version === 'string') {
    addPackageVersion(versionsByPackage, packageName, dependency.version);
  }
  walkDependencies(dependency, versionsByPackage);
}

/** Walk one dependency section from `pnpm ls` and record installed versions.
 * @param {Record<string, unknown> | undefined} section Dependency section keyed by package name. @param {Map<string, Set<string>>} versionsByPackage Collected versions keyed by package name.
 * @returns {void} @example const versions = new Map(); walkDependencySection({ validator: { version: '13.15.23' } }, versions); console.log(versions.get('validator').has('13.15.23')); // true
 */
function walkDependencySection(section, versionsByPackage) {
  if (!section || typeof section !== 'object') {
    return;
  }
  for (const [packageName, dependency] of Object.entries(section)) {
    processDependencyEntry(packageName, dependency, versionsByPackage);
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

/** Return `true` when a value is a valid `pnpm ls` dependency tree node
 * (a non-null, non-array plain object).
 * @param {unknown} value Value to test.
 * @returns {boolean}
 * @example isValidTreeNode({ dependencies: {} }); // true
 * @example isValidTreeNode(null);                 // false
 * @example isValidTreeNode([]);                   // false
 */
function isValidTreeNode(value) {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Build the installed package-version map from parsed `pnpm ls` output.
 * @param {Record<string, unknown> | Record<string, unknown>[] | undefined} packageTrees Parsed `pnpm ls` output as one tree or many. @returns {Map<string, Set<string>>} Installed versions keyed by package name. @example const versions = buildVersionMap([{ dependencies: { validator: { version: '13.15.23' } } }]); console.log(versions.get('validator').has('13.15.23')); // true
 */
export function buildVersionMap(packageTrees) {
  const versionsByPackage = new Map();
  const trees = Array.isArray(packageTrees) ? packageTrees : [packageTrees];
  for (const tree of trees) {
    if (!isValidTreeNode(tree)) {
      throw new TypeError('pnpm ls returned an invalid dependency tree payload.');
    }
    walkDependencies(tree, versionsByPackage);
  }
  return versionsByPackage;
}

/** Load parsed package trees from `pnpm ls`.
 * @param {object} auditIo Audit IO adapter.
 * @param {(result: { error?: Error, signal?: string | null, status?: number | null }, commandLabel: string) => number} assertCompletedProcess Process completion validator.
 * @returns {Record<string, unknown> | unknown[]} Parsed package-tree payload.
 * @example loadPackageTrees(auditIo, assertCompletedProcess); // [{ dependencies: {} }]
 */
export function loadPackageTrees(auditIo, assertCompletedProcess) {
  const result = auditIo.spawnSync('pnpm', LIST_ARGS, {
    encoding: 'utf8',
    maxBuffer: COMMAND_MAX_BUFFER,
    stdio: ['ignore', 'pipe', 'inherit'],
  });
  const status = assertCompletedProcess(result, 'pnpm ls');
  if (status !== 0) {
    throw new Error(`pnpm ls failed without producing a dependency tree (exit status ${status}).`);
  }
  const stdout = result.stdout?.trim();
  if (!stdout) {
    throw new Error('pnpm ls failed without producing a dependency tree.');
  }
  return parseJsonOutput(stdout, 'pnpm ls');
}

/** Convert a version map to the sorted object expected by bulk advisory lookups.
 * @param {Map<string, Set<string>>} versionsByPackage Installed versions keyed by package name.
 * @returns {Record<string, string[]>} Sorted installed versions keyed by package name.
 * @example serializeVersionMap(new Map([['validator', new Set(['13.15.23'])]])); // { validator: ['13.15.23'] }
 */
function serializeVersionMap(versionsByPackage) {
  return Object.fromEntries(
    [...versionsByPackage.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([packageName, versions]) => [packageName, [...versions].sort()]),
  );
}

/** Collect installed package versions from `pnpm ls` for bulk advisory lookups.
 * Throws when `pnpm ls` fails, is signalled, or returns invalid JSON.
 * @param {object} auditIo Audit IO adapter.
 * @param {(result: { error?: Error, signal?: string | null, status?: number | null }, commandLabel: string) => number} assertCompletedProcess Process completion validator.
 * @returns {Record<string, string[]>} Sorted installed versions keyed by package name.
 * @example collectInstalledPackageVersions(auditIo, assertCompletedProcess); // { validator: ['13.15.23'] }
 */
export function collectInstalledPackageVersions(auditIo, assertCompletedProcess) {
  return serializeVersionMap(buildVersionMap(loadPackageTrees(auditIo, assertCompletedProcess)));
}

/** Extract a GitHub advisory identifier from an advisory URL.
 * @param {unknown} advisoryUrl Advisory URL from pnpm or npm audit output.
 * @returns {string | undefined} Matching GHSA identifier when one is present. @example extractGithubAdvisoryId('https://github.com/advisories/GHSA-vghf-hv5q-vc2g'); // 'GHSA-vghf-hv5q-vc2g'
 */
function extractGithubAdvisoryId(advisoryUrl) {
  if (typeof advisoryUrl !== 'string') {
    return undefined;
  }
  const match = advisoryUrl.match(/GHSA-([0-9a-z]{4})-([0-9a-z]{4})-([0-9a-z]{4})/i);
  if (!match) {
    return undefined;
  }
  const [, first, second, third] = match;
  return `GHSA-${first.toLowerCase()}-${second.toLowerCase()}-${third.toLowerCase()}`;
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

/** Return `true` when a value is a plain (non-array, non-null) object.
 * @param {unknown} value Value to test. @returns {boolean}
 * @example isPlainAdvisoryObject({ id: 1 }); // true
 */
function isPlainAdvisoryObject(value) { return typeof value === 'object' && value !== null && !Array.isArray(value); }

/** Validate and merge one advisory into the shared accumulator.
 * @param {string} packageName Package name from the bulk advisory payload.
 * @param {unknown} advisory Raw advisory object; must be a plain object.
 * @param {number} index Position within the package advisory array.
 * @param {Record<string, unknown>} advisories Accumulator mutated in place.
 * @returns {void}
 */
function mergeOneAdvisory(packageName, advisory, index, advisories) {
  if (!isPlainAdvisoryObject(advisory)) {
    throw new Error(`Invalid advisory for package ${packageName} at index ${index}: expected object`);
  }
  const { key, githubAdvisoryId } = deriveAdvisoryKey(packageName, advisory);
  if (Object.hasOwn(advisories, key)) {
    return;
  }
  advisories[key] = {
    ...advisory,
    package_name: packageName,
  };
  if (githubAdvisoryId != null) {
    advisories[key].github_advisory_id = githubAdvisoryId;
  }
}

/** Merge advisories for one package into the shared accumulator.
 * @param {string} packageName Package name from the bulk advisory payload. @param {unknown[]} packageAdvisories Validated array of raw advisory objects. @param {Record<string, unknown>} advisories Accumulator mutated in place.
 * @returns {void} @example const advisories = {}; addPackageAdvisories('validator', [{ id: 100000, url: 'https://github.com/advisories/GHSA-vghf-hv5q-vc2g', title: 'Validator SSRF' }], advisories); console.log(advisories['GHSA-vghf-hv5q-vc2g'].package_name); // 'validator'
 */
function addPackageAdvisories(packageName, packageAdvisories, advisories) {
  for (const [index, advisory] of packageAdvisories.entries()) {
    mergeOneAdvisory(packageName, advisory, index, advisories);
  }
}

/** Validate and merge one package's advisory array into the accumulator.
 * @param {string} packageName Package name from the bulk advisory payload.
 * @param {unknown} packageAdvisories Raw value for this package; must be an array.
 * @param {Record<string, unknown>} advisories Accumulator mutated in place.
 * @returns {void}
 */
function mergePackageAdvisories(packageName, packageAdvisories, advisories) {
  if (!Array.isArray(packageAdvisories)) {
    throw new TypeError(
      `Invalid bulk advisory entry for package ${packageName}: expected array, received ${JSON.stringify(packageAdvisories)}`,
    );
  }
  addPackageAdvisories(packageName, packageAdvisories, advisories);
}

/** Normalize bulk advisory responses into the shared advisory object shape.
 * @param {Record<string, unknown> | undefined} bulkPayload Bulk advisory payload keyed by package name.
 * @returns {Record<string, unknown>} Deduplicated advisories keyed by GHSA identifier or package fallback. @example normalizeBulkAdvisories({ validator: [{ id: 100000, url: 'https://github.com/advisories/GHSA-vghf-hv5q-vc2g' }] }); // { 'GHSA-vghf-hv5q-vc2g': { github_advisory_id: 'GHSA-vghf-hv5q-vc2g', package_name: 'validator', id: 100000, url: 'https://github.com/advisories/GHSA-vghf-hv5q-vc2g' } }
 */
export function normalizeBulkAdvisories(bulkPayload) {
  if (!isPlainAdvisoryObject(bulkPayload)) {
    throw new TypeError('Invalid bulk advisory payload: expected an object keyed by package name.');
  }
  const advisories = {};
  for (const [packageName, packageAdvisories] of Object.entries(bulkPayload)) {
    mergePackageAdvisories(packageName, packageAdvisories, advisories);
  }

  return advisories;
}
