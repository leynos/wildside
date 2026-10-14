# Adopt sha2 0.11 with explicit hex digest encoding (#446)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` are maintained as work proceeds.

Status: IN PROGRESS

No `PLANS.md` file exists in this repository root.

## Purpose / Big picture

`sha2` 0.11 finalizes to `hybrid_array::Array<u8, _>`, which derefs to `[u8]`
but implements neither `core::fmt::LowerHex` nor `std::io::Write`. One backend
call site formatted a digest with `{:x}`, so the crate would stop compiling the
moment Dependabot proposed the bump. Issue [#446] adopts 0.11 now and renders
the digest the way the rest of the backend already does.

Pull request [#448] carries the change. This plan tracks the review-feedback
round on that PR.

Observable success:

- `make check-fmt`, `make lint`, `make typecheck`, and `make test` all succeed.
- PR [#448] reaches equilibrium and merges.

[#446]: https://github.com/leynos/wildside/issues/446
[#448]: https://github.com/leynos/wildside/pull/448

## Constraints

- Fix underlying issues rather than silencing or suppressing lints.
- Keep changes minimal and evidence-based; skip findings that no longer hold.
- Re-run every gate after each change and commit; push after each commit.
- Do not run gates in parallel: this environment uses build caching, and
  sequential execution is the best way to benefit from it.
- Delegate full commit-gate runs to `scrutineer`.

## Tolerances (exception triggers)

- If a requested change proves unwarranted against the current code, stop and
  record the reason rather than implementing it.
- If full gates fail after three focused repair iterations, stop and escalate.
- If a change would require weakening an existing quality gate, stop and
  escalate.
- If scope grows beyond the PR's stated purpose, stop and escalate.

## Risks

- The compile-fail trybuild driver may exceed nextest's 60 s slow-timeout on a
  cold runner. Mitigated below by excluding the new driver from the nextest
  lane and running it in the dedicated `cargo test` compile-fail step.
- Re-blessing `.stderr` fixtures against a changed toolchain can produce churn
  unrelated to this change. Mitigated by blessing only after confirming the
  fixtures still fail to compile for the intended reason.

## Progress

- [x] (2026-10-14) Collected review feedback from PR [#448].
- [x] (2026-10-14) Rebased onto `origin/main` (`f315df5`); dropped the h2
      audit commit as superseded (see Decision Log).
- [x] Fix Codex P1: exclude the new trybuild driver from the nextest lane.
- [x] Fix CodeRabbit inline: assert a digest containing a leading-zero byte.
- [x] Fix CodeRabbit inline: correct the `hex::encode` call-site list in the
      developers' guide.
- [x] Diagnose the 21 `documentation_gate_behaviour_test.py` `TS2688` failures
      (a stale `/tmp/node_modules` tree, not the repository) and harden the
      probe's `tsconfig` against it.
- [x] Address CodeRabbit pre-merge warnings where warranted.
- [x] (2026-10-14) Ran all gates via `scrutineer`. `make typecheck` and
      `make test` passed; `make check-fmt` and `make lint` failed on two
      unrelated defects, both now repaired:
  - `check-fmt`: two Markdown files needed reformatting; fixed with
    `mdtablefix`.
  - `lint`: `checkmake`'s `maxbodylength` rule rejected the six-line body of the
    `fmt` target. Verified pre-existing by running `checkmake` against
    `git show origin/main:Makefile`: the same single violation at the same line.
    Fixed by splitting the two Ruff lines into a new `fmt-python` target,
    following the repository's own precedent and its existing
    `check-fmt-python` seam.
- [x] Repair the third `make lint` defect: `lint-actions` (`yamllint`)
      rejected `ci.yml`'s nextest filter line at 139 characters once the third
      binary name joined it. Not satisfiable by reflow; see the finding below.
- [x] Decide the regenerated `typos.toml`: left out of this commit. It is
      pre-existing drift, and the gate that rewrites it cannot fail on it; see
      the finding below.
- [x] (2026-10-14) Established that the three review fixes exist only in the
      working tree. `168e799` still carries the old weak
      length-and-character-class assertion, the flawed "Every call site follows
      this convention" paragraph that lists `payload.rs`, and no
      `NEXTEST_EXCLUSION` in `ci.yml`. Every reply must therefore cite the new
      commit, not `168e799`; citing the pushed commit before it exists would be
      a false claim.
- [ ] Re-run the full gate set on the repaired tree.
- [ ] Push, then shepherd PR [#448] to merge via `pr-babysitting`.

## Surprises & Discoveries

- **The `ci.yml` nextest filter line cannot be reflowed to fit yamllint.**
  Adding `binary(sha2_digest_formatting_compile_fail)` to the `-E` expression
  made the line 139 characters, over the 120 the repository's `.yamllint`
  allows. Three separate constraints rule out every in-place rewrite, and each
  was verified rather than assumed:

  1. The expression itself is 122 characters. `checks` in
     `yamllint/rules/line_length.py` exempts an over-long line only when it
     contains **no space at all** after the indent (`find(' ', start, end) ==
     -1`); `allow-non-breakable-words` is therefore unreachable for a value
     containing spaces. With `-E '` + `'` and any indent, the line is at least
     127 characters. No quoting or indent choice fits.

  2. A shell continuation inside a *quoted* value cannot work either.
     `shell_commands.command_words` joins a script with
     `script.replace("\\\n", " ")` — it substitutes a **space** — while bash
     removes the newline entirely. Measured: a wrapped double-quoted
     expression parses back as
     `...compile_fail_tests)                | binary(sha2...)`, with the
     continuation line's indent retained as literal spaces, so it never equals
     the pinned `NEXTEST_EXCLUSION`. Bash would accept it; the contract would
     not. (This was caught by experiment before it reached a commit. My first
     attempt wrapped the expression inside single quotes, which bash would have
     read as an unterminated string with the following `|` lines as pipes —
     that would have broken CI.)

  3. Passing the expression through an environment variable keeps the line
     short but changes what the contract sees.
     `duplicate_test_lane_test.py` calls `shell.option_value(command, "-E")`,
     which returns the literal shell word, so `-E "$NEXTEST_EXCLUSION"` reads
     as the string `$NEXTEST_EXCLUSION` and fails the pinned comparison.

  The resolution is to keep the expression where the contract reads it and move
  the *line*: the mechanism is recorded in the Decision Log.

- **The spelling gate rewrites `typos.toml`, and cannot fail on it.** Running
  `make markdownlint` (which depends on `spelling`) left a modification to the
  tracked, generated `typos.toml`: one `extend-ignore-re` entry changed from a
  hand-era pattern to the v0.1.3 form. Three facts settled what to do with it:

  1. `typos-config-builder gate --help` states it "always runs in write mode,
     so a live dictionary edit is picked up rather than reported as drift".
     There is no check mode. A stale tracked `typos.toml` therefore cannot fail
     CI; the gate overwrites it and proceeds.
  2. The regenerated file is a fixed point: a second full gate run produced a
     byte-identical file.
  3. The drift is pre-existing and repo-wide, not ours. The pin moved to v0.1.3
     in `3cefbe8` (2026-10-02) *without* regenerating `typos.toml`; the file was
     last committed in `051c8e3` (2026-10-05) by an unrelated dependency bump,
     which re-committed the stale value.

  AGENTS.md's documented repair route — edit `typos.local.toml`, then run
  `make spelling-config-write` — is not available: neither
  `spelling-config-write` nor `spelling-config` exists as a Makefile target.

  The change is therefore left out of this commit. It is unrelated to issue
  [#446], it is not required for any gate to pass, and PR [#448] already
  attracted an "Out of Scope Changes" warning from CodeRabbit. Recording it
  here is the honest alternative to smuggling an unrelated generated-file
  change into a digest-encoding commit.

- **`main` independently resolved the h2 advisory.** Commit `9ad5130` ("Clear
  the h2 advisory blocking the audit gate") was written when `main` still
  carried the failing `RUSTSEC-2026-0258`. By rebase time `origin/main` already
  carried an equivalent, larger change: the same
  `CARGO_AUDIT_IGNORES := --ignore RUSTSEC-2023-0071 --ignore RUSTSEC-2026-0258`,
  the same `EXPECTED_CARGO_AUDIT` refactor in
  `scripts/makefile-audit.test.mjs`, and a longer Makefile rationale
  referencing issue [#472](https://github.com/leynos/wildside/issues/472) with
  a 2026-10-03 review date. The commit conflicted in all three of its files,
  and every conflict hunk was structural — main's content is the same with
  different naming. The commit was therefore skipped during rebase rather than
  hand-merged.

- **The Codex P1 was the actual cause of the failing CI `build` job.** The job
  log for run `32646722832` ends with:

  ```text
  TERMINATING [> 60.000s] (─────────) backend::sha2_digest_formatting_compile_fail sha2_pre_migration_digest_patterns_do_not_compile
  (test timed out)
  Summary [ 121.376s] 1300/1342 tests run: 1299 passed, 1 timed out, 4 skipped
  error: test run failed
  ```

  The trybuild driver ran inside the nextest lane and was killed by the 60 s
  `slow-timeout`, exactly as the finding predicted. No production code failed;
  the fixture driver simply needed the dedicated `cargo test` step.

- **`sha256(b"abc")` already contains a leading-zero byte.** The digest is
  `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`; byte 29 is
  `0x00`, rendered as `00` at hex offset 58. So the existing
  `sha256_file_matches_known_vectors` `::abc` case already pins leading-zero
  preservation, and the new assertion in the fixed-width test can reuse it.

- **The 21 `TS2688` failures came from outside the repository.** After the
  rebase, `make test-workflow-contracts` went red on
  `tests/workflow_contracts/documentation_gate_behaviour_test.py` with
  `TS2688: Cannot find type definition file for 'minimatch'. The file is in the
  program because: Entry point for implicit type library 'minimatch'`
  for the `frontend-pwa` surface.

  Every file implicated was byte-identical to `origin/main`, and no
  configuration in the repository names `minimatch`: `pnpm-lock.yaml` has zero
  references and no `package.json` depends on it. The cause was a stale
  `/tmp/node_modules` tree — 1031 packages, written 2026-10-10, last touched
  2026-10-12 — left by some other project.

  An absent `types` key makes the compiler collect *every*
  `node_modules/@types` directory between the tsconfig and the filesystem root
  and include each as an implicit type library. The fixtures are written to
  pytest's `tmp_path`, which is `/tmp/pytest-of-leynos/…`, so they inherited
  `/tmp/node_modules/@types`, including a half-installed `@types/minimatch`
  that has a `package.json` but no `index.d.ts`. Confirmed by walking the
  ancestors of a clean temporary root, which have no `node_modules/@types` at
  all, and by re-running the exact probe with `"types": []`: before, one
  `TS2688` error; after, exit 0 and no output.

  This is a latent hermeticity bug in the contract, not just a local accident:
  the fixture's compilation depended on whatever happened to sit above the
  temporary directory. The fix is `"types": []` in the fixture's compiler
  options. The probe declares no ambient types, so including none is the
  accurate description of the program.

  The step only became reachable here because `origin/main` added the test in
  `56eba4a` *after* PR [#448]'s last CI run: the `build` job for run
  `32646722832` has no `TypeDoc documentation gate` step at all, which is why
  `Workflow contract tests` passed then and would fail now.

- **A true `ingest-osm` CLI end-to-end test is not cheaply available.** The
  binary's OSM source adapter (`WildsideDataOsmSourceRepository`) delegates to
  `wildside-data::ingest_osm_pbf_report`, an external git dependency
  (`leynos/wildside-engine`). Exercising the command boundary therefore needs a
  valid `.osm.pbf` fixture plus an embedded PostgreSQL instance. The existing
  `backend/tests/osm_ingestion_bdd.rs` sidesteps this: it replaces the source
  adapter with a fixture double and drives the *domain service*, not the binary.

## Decision Log

- Decision: skip commit `9ad5130` ("Clear the h2 advisory blocking the audit
  gate") during the rebase instead of resolving its conflicts. Rationale:
  `origin/main` already carries the same ignore, the same audit-test refactor,
  and a better-documented Makefile rationale. Hand-merging would have
  re-introduced duplicate or contradictory text for no functional gain, and the
  change is out of scope for issue [#446] — a point CodeRabbit raised
  independently. Date: 2026-10-14.

- Decision: treat the h2 commit's removal as resolving the CodeRabbit
  "Security And Privacy" error and the "Out of Scope Changes" warning.
  Rationale: both findings were about that commit's contents, which no longer
  exist on this branch after the rebase. Date: 2026-10-14.

- Decision: fix `documentation_gate_behaviour_test.py` in this PR rather than
  reporting the `TS2688` failures as a pre-existing main defect. Rationale: the
  repository change is two lines in a test fixture, and the alternative is a
  permanently unreliable contract. The failure is not attributable to the host
  alone — it is a real hermeticity bug in the fixture, which depended on
  `node_modules/@types` directories it never declared. The host's stale
  `/tmp/node_modules` only made a latent flaw visible. Fixing it here also
  unblocks `make test` on `main`, where the same failure would occur. Date:
  2026-10-14.

- Decision: reply to the review threads using the allocated bot identities
  (`buzzybee-df12`, `pandalump`, `wafflecat-df12`) rather than `leynos`.
  Rationale: the user allocated these accounts specifically so that review
  replies do not consume the `leynos` account's rate-limit budget against
  CodeRabbit. The tokens live at `~/.<name>-token`. Date: 2026-10-14.

- Decision: hold the third binary name in the `Rust tests` step's `env` as
  `NEXTEST_EXCLUSION`, and pass it to nextest by reference as
  `-E "$NEXTEST_EXCLUSION"`. Rationale: the enumerated expression is 122
  characters and the workflow's linter allows 120, and no in-place rewrite
  fits. The linter's `allow-non-breakable-words` escape needs a line with no
  space at all; a shell continuation inside a quoted value is read as *space* by
  `shell_commands.command_words` but as *nothing* by bash, so the parsed value
  never equals the pin; and `option_value` returns the literal shell word, so
  the reference must be pinned as a reference rather than as a value. An
  alternative `binary(~compile_fail)` pattern was rejected because it would
  silently absorb a future compile-fail suite into a lane that never runs it.
  Name `NEXTEST_EXCLUSION` was checked against the nextest binary: it does not
  appear there, and nextest reads only fully-spelled `NEXTEST_*` names, so it
  is inert to the runner it sits beside. Date: 2026-10-14.

- Decision: do not reflow the ci.yml filter into a backslash-continued
  argument list, as suggested during an earlier gate review. Rationale:
  measured, not assumed. `shell_commands.command_words` joins a script with
  `script.replace("\\\n", " ")` — a space — while bash removes the newline
  entirely, so the continuation line's indent survives into the parsed value.
  Bash would accept the line; the contract would fail it. Withdrawn rather than
  applied. Date: 2026-10-14.

## Outcomes & Retrospective

Populated on completion.
