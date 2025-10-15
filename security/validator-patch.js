/** @file Helpers for verifying the locally patched validator dependency. */

import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

function resolveRulesetRequire() {
  const workspaceRequire = createRequire(new URL('../frontend-pwa/package.json', import.meta.url));
  const orvalRequire = createRequire(workspaceRequire.resolve('orval/package.json'));
  const coreRequire = createRequire(orvalRequire.resolve('@orval/core/package.json'));
  return createRequire(coreRequire.resolve('@ibm-cloud/openapi-ruleset/package.json'));
}

export function resolveValidatorPath() {
  const rulesetRequire = resolveRulesetRequire();
  return rulesetRequire.resolve('validator/lib/isURL');
}

export function isValidatorPatched() {
  const validatorPath = resolveValidatorPath();
  const contents = readFileSync(validatorPath, 'utf8');
  return contents.includes("var firstColon = url.indexOf(':');");
}
