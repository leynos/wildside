/** @file Helpers for verifying the hardened validator dependency. */

import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';

import { VALIDATOR_MIN_SAFE_VERSION } from './constants.js';

/**
 * Resolve the absolute path to a dependency's package.json without relying on
 * subpath exports. Some dependencies restrict access to package.json via the
 * "exports" field; we instead resolve the entry point and walk upwards.
 *
 * @param {NodeRequire} requireFn createRequire instance anchored at a package.
 * @param {string} specifier Module identifier to resolve within the package.
 * @returns {string} Absolute filesystem path to the package.json.
 * @throws {Error} When package.json cannot be located.
 */
function resolvePackageJsonPath(requireFn, specifier) {
  const entryPoint = requireFn.resolve(specifier);
  let current = dirname(entryPoint);

  while (!existsSync(join(current, 'package.json'))) {
    const parent = dirname(current);
    if (parent === current) {
      throw new Error(`Could not locate package.json for ${specifier}`);
    }
    current = parent;
  }

  return join(current, 'package.json');
}

/**
 * Resolve the require function anchored at @ibm-cloud/openapi-ruleset.
 *
 * Resolution chain: frontend-pwa → orval → @orval/core → @ibm-cloud/openapi-ruleset.
 * The try/catch clarifies failures if any dependency along the chain changes.
 *
 * @throws {Error} If the dependency chain cannot be resolved.
 */
function resolveRulesetRequire() {
  try {
    const workspaceRequire = createRequire(
      new URL('../frontend-pwa/package.json', import.meta.url),
    );
    const orvalRequire = createRequire(resolvePackageJsonPath(workspaceRequire, 'orval'));
    const coreRequire = createRequire(resolvePackageJsonPath(orvalRequire, '@orval/core'));
    return createRequire(resolvePackageJsonPath(coreRequire, '@ibm-cloud/openapi-ruleset'));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to resolve validator dependency chain: ${message}`, {
      cause: error,
    });
  }
}

export function resolveValidatorPackageJsonPath() {
  const rulesetRequire = resolveRulesetRequire();
  return resolvePackageJsonPath(rulesetRequire, 'validator');
}

export function resolveValidatorPath() {
  const rulesetRequire = resolveRulesetRequire();
  return rulesetRequire.resolve('validator/lib/isURL');
}

function normaliseVersionTuple(version) {
  const parts = String(version)
    .split('.')
    .slice(0, 3)
    .map((part) => Number.parseInt(part, 10) || 0);
  while (parts.length < 3) {
    parts.push(0);
  }
  return parts;
}

function isAtLeastVersion(version, minimum) {
  const current = normaliseVersionTuple(version);
  const target = normaliseVersionTuple(minimum);

  for (let index = 0; index < target.length; index += 1) {
    if (current[index] > target[index]) return true;
    if (current[index] < target[index]) return false;
  }
  return true;
}

/**
 * Check if the validator dependency includes the upstream fix for the current
 * GitHub advisory. Preference is given to the package version to avoid brittle
 * string matching, with a fallback to the legacy patch marker for older builds
 * should the workspace temporarily pin a pre-patch version.
 *
 * @returns {boolean} True when the validator mitigation is detected.
 */
export function isValidatorPatched() {
  const packageJsonPath = resolveValidatorPackageJsonPath();
  const validatorPath = resolveValidatorPath();
  const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
  const validatorVersion = packageJson.version;

  if (validatorVersion && isAtLeastVersion(validatorVersion, VALIDATOR_MIN_SAFE_VERSION)) {
    return true;
  }

  const contents = readFileSync(validatorPath, 'utf8');
  return contents.includes("var firstColon = url.indexOf(':');");
}
