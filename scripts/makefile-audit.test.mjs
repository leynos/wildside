/** @file Functional dry-run tests for the Makefile audit target contracts. */

import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { describe, expect, it } from 'vitest';

const execFileAsync = promisify(execFile);
const repositoryRoot = new URL('../', import.meta.url);

/**
 * Ask Make to print a target's execution plan without running the recipes.
 * @param {string} target Make target to dry-run.
 * @returns {Promise<string>} Commands Make would execute for the target.
 */
async function dryRunMake(target) {
  const { stdout } = await execFileAsync('make', ['--dry-run', '--always-make', target], {
    cwd: repositoryRoot,
  });
  return stdout;
}

describe('Makefile audit targets', () => {
  it('executes the aggregate audit target through node and Rust audits', async () => {
    const stdout = await dryRunMake('audit');

    expect(stdout).toContain('pnpm -r --if-present run audit');
    expect(stdout).toContain('pnpm run audit:validate');
    expect(stdout).toContain('cargo audit --file Cargo.lock --ignore RUSTSEC-2023-0071');
  });

  it('does not reinstall node dependencies inside audit-node', async () => {
    const stdout = await dryRunMake('audit-node');

    expect(stdout).toContain('pnpm -r --if-present run audit');
    expect(stdout).toContain('pnpm run audit:validate');
    expect(stdout).not.toContain('pnpm -r install');
  });

  it('checks cargo-audit availability before running the Rust audit', async () => {
    const stdout = await dryRunMake('rust-audit');

    expect(stdout).toContain('command -v cargo-audit');
    expect(stdout).toContain('cargo-audit is required');
    expect(stdout).toContain('cargo-audit@0.22.1');
  });

  it('runs cargo audit against Cargo.lock with configured ignores', async () => {
    const stdout = await dryRunMake('rust-audit');

    expect(stdout).toContain('cargo audit --file Cargo.lock --ignore RUSTSEC-2023-0071');
  });
});
