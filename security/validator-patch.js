/** @file Helpers for verifying the locally patched validator dependency. */

import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';

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
    return createRequire(
      resolvePackageJsonPath(coreRequire, '@ibm-cloud/openapi-ruleset'),
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to resolve validator dependency chain: ${message}`, {
      cause: error,
    });
  }
}

export function resolveValidatorPath() {
  const rulesetRequire = resolveRulesetRequire();
  return rulesetRequire.resolve('validator/lib/isURL');
}

/**
 * Check if the validator patch marker is present in the vendored module.
 *
 * The detection relies on an exact snippet introduced by the patch. This is
 * brittle but suffices until upstream releases a fixed build.
 *
 * @returns {boolean} True when the validator patch is detected.
 */
export function isValidatorPatched() {
  const validatorPath = resolveValidatorPath();
  const contents = readFileSync(validatorPath, 'utf8');
  return contents.includes("var firstColon = url.indexOf(':');");
}
