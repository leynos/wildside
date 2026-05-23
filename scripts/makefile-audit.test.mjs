/** @file Tests the Makefile audit target contracts. */

import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';

const makefilePath = new URL('../Makefile', import.meta.url);

/**
 * Read the repository Makefile for contract checks.
 *
 * @returns {Promise<string>} The Makefile source.
 */
async function readMakefile() {
  return readFile(makefilePath, 'utf8');
}

/**
 * Extract a Make target recipe body from Makefile source.
 *
 * @param {string} source - The Makefile source.
 * @param {string} target - The target name to extract.
 * @returns {string} The target recipe body.
 */
function extractTarget(source, target) {
  const match = source.match(
    new RegExp(`^${target}:[^\\n]*(?:\\n\\t[^\\n]*)*`, 'm'),
  );
  return match?.[0] ?? '';
}

describe('Makefile audit targets', () => {
  it('wires the aggregate audit target through node and Rust audits', async () => {
    const makefile = await readMakefile();

    expect(makefile).toMatch(/^audit: audit-node rust-audit$/m);
  });

  it('does not reinstall node dependencies inside audit-node', async () => {
    const makefile = await readMakefile();
    const target = extractTarget(makefile, 'audit-node');

    expect(target).toContain('audit-node: deps');
    expect(target).toContain('pnpm -r --if-present run audit');
    expect(target).toContain('pnpm run audit:validate');
    expect(target).not.toContain('pnpm -r install');
  });

  it('checks cargo-audit availability before running the Rust audit', async () => {
    const makefile = await readMakefile();
    const target = extractTarget(makefile, 'rust-audit');

    expect(target).toContain('command -v cargo-audit');
    expect(target).toContain('cargo-audit is required');
    expect(target).toContain('cargo-audit@0.22.1');
  });

  it('runs cargo audit against Cargo.lock with the configured ignores', async () => {
    const makefile = await readMakefile();
    const target = extractTarget(makefile, 'rust-audit');

    expect(makefile).toMatch(
      /^CARGO_AUDIT_IGNORES := --ignore RUSTSEC-2023-0071$/m,
    );
    expect(target).toContain(
      '$(CARGO) audit --file Cargo.lock $(CARGO_AUDIT_IGNORES)',
    );
  });
});
