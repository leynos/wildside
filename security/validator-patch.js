/** @file Helpers for verifying the locally patched validator dependency. */

import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

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
    const workspaceRequire = createRequire(new URL('../frontend-pwa/package.json', import.meta.url));
    const orvalRequire = createRequire(workspaceRequire.resolve('orval/package.json'));
    const coreRequire = createRequire(orvalRequire.resolve('@orval/core/package.json'));
    return createRequire(coreRequire.resolve('@ibm-cloud/openapi-ruleset/package.json'));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to resolve validator dependency chain: ${message}`);
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
