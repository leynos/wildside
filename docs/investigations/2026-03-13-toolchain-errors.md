# Toolchain error investigation: 2026-03-13

## Scope

This report documents three failures observed while validating the backend
implementation for roadmap item `3.5.4`:

- `cargo test -p backend --test user_interests_revision_conflicts_bdd`
- `make lint`
- `make test`

The goal here is to capture the exact evidence, the relevant code paths, what
was ruled out, and the most likely diagnoses supported by the investigation.

Historical note: later on 2026-03-13 the repository switched from a
repo-local `backend` `pg_worker` binary to the `pg_worker` binary published by
`pg-embed-setup-unpriv`. Code-path references below describe the wiring that
was in place when these failures were captured.

## Environment snapshot

The current shell and toolchain state at the time of this investigation was:

```text
1  rustc: rustc 1.96.0-nightly (3102493c7 2026-03-12)
2  cargo: cargo 1.96.0-nightly (90ed291a5 2026-03-05)
3  clippy-driver: clippy 0.1.96 (3102493c71 2026-03-12)
4  toolchain: nightly-x86_64-unknown-linux-gnu (overridden by '/home/user/project/rust-toolchain.toml')
5  devnull: path=/dev/null type=character special file mode=666 owner=root:root major=1 minor=3
```

Evidence source: local command output captured with line numbers during this
investigation.

The repository pins the Rust toolchain via `rust-toolchain.toml`:

```toml
[toolchain]
channel = "nightly"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

The toolchain binaries also existed and resolved correctly at inspection time:

```text
1  -rwxr-xr-x 1 root root 20865872 Mar 13 08:07 /root/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin/clippy-driver
2  -rwxr-xr-x 1 root root   645192 Mar 13 08:08 /root/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin/rustc
3  ---
4      linux-vdso.so.1
5      librustc_driver-e0f153d46f434a65.so => /root/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/...
6      libdl.so.2 => /lib/x86_64-linux-gnu/libdl.so.2
7      librt.so.1 => /lib/x86_64-linux-gnu/librt.so.1
8      libpthread.so.0 => /lib/x86_64-linux-gnu/libpthread.so.0
9      libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6
10     libLLVM.so.22.1-rust-1.96.0-nightly => /root/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/...
11     /lib64/ld-linux-x86-64.so.2
```

This does not prove the binaries were always executable at failure time, but it
does rule out the simplest explanation that the files were merely absent when
inspected.

## Relevant code paths

The failure sites line up with the following repository code:

- `Makefile:89-92`
  - `lint-rust` runs `cargo doc`, then `cargo clippy`, then `whitaker`.
- `Makefile:159-167`
  - `test-rust` depends on `prepare-pg-worker`.
  - `prepare-pg-worker` builds `backend` binary `pg_worker`.
- `backend/tests/support/cluster_skip.rs:25-30`
  - cluster bootstrap failures are escalated via
    `panic!("Test cluster setup failed: ...")`.
- `backend/tests/user_interests_revision_conflicts_bdd.rs:39-56`
  - the BDD suite starts embedded Postgres in the `Given
    db-present startup mode backed by embedded postgres` step.

Relevant excerpts:

```make
89  lint-rust:
90      RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" cargo doc --workspace --no-deps
91      cargo clippy --workspace --all-targets --all-features -- $(RUST_FLAGS)
92      $(RUST_FLAGS_ENV) whitaker --all -- --manifest-path Cargo.toml --workspace --all-targets --all-features
...
159 test-rust: workspace-sync prepare-pg-worker
160     PG_EMBEDDED_WORKER=$(PG_WORKER_PATH) \
161     NEXTEST_TEST_THREADS=$(NEXTEST_TEST_THREADS) $(RUST_FLAGS_ENV) \
162     cargo nextest run --workspace --all-targets --all-features --no-fail-fast
...
166 prepare-pg-worker:
167     $(RUST_FLAGS_ENV) cargo build -p backend --bin pg_worker
```

```rust
25 pub fn handle_cluster_setup_failure<T>(reason: impl std::fmt::Display) -> Option<T> {
26     if should_skip_test_cluster() {
27         eprintln!("SKIP-TEST-CLUSTER: {reason}");
28         None
29     } else {
30         panic!("Test cluster setup failed: {reason}. Set SKIP_TEST_CLUSTER=1 to skip.");
31     }
32 }
```

```rust
39 #[given("db-present startup mode backed by embedded postgres")]
40 fn db_present_startup_mode_backed_by_embedded_postgres(world: &mut World) {
41     match setup_db_context() {
42         Ok(db) => {
...
52         Err(error) => {
53             let _ = handle_cluster_setup_failure::<()>(error.as_str());
54             world.skip_reason = Some(error);
55         }
56     }
57 }
```

## Failure 1: BDD test fails before scenario logic

### Failure 1 command

```text
cargo test -p backend --test user_interests_revision_conflicts_bdd
```

### Failure 1 primary evidence

The recorded log shows the failure occurs during embedded PostgreSQL bootstrap,
before any scenario assertions execute:

```text
18  ---- missing_expected_revision_after_preferences_exist_returns_a_conflict stdout ----
19  The application panicked (crashed).
20  Message:  Test cluster setup failed: shared cluster initialisation
21  previously failed: BootstrapError { kind: Other, report:
22     0: bootstrap failed: BootstrapError { kind: Other, report:
23           0: postgresql_embedded::setup() failed
24              stdout:
25              stderr: Error:
26                 0: postgresql_embedded::setup failed
27                 1: Command error: stdout=The files belonging to this
28                 database system will be owned by user "nobody".
...
41                    running bootstrap script ... ok
42                    performing post-bootstrap initialization ... ; stderr=sh: 1: cannot create /dev/null: Permission denied
43                    sh: 1: cannot create /dev/null: Permission denied
44                    sh: 1: cannot create /dev/null: Permission denied
45                    sh: 1: cannot create /dev/null: Permission denied
...
65        Location:
66           /root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/pg-embed-setup-unpriv-0.5.0/src/worker_process/output.rs:15
...
71  Location:
72     /root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/pg-embed-setup-unpriv-0.5.0/src/test_support/shared_singleton.rs:113
...
75  Run with RUST_BACKTRACE=full to include source snippets. }. Set SKIP_TEST_CLUSTER=1 to skip.
76  Location: backend/tests/support/cluster_skip.rs:30
```

The same log also shows the BDD framework reporting failure at the first `Given`
step:

```text
80  The application panicked (crashed).
81  Message:  Step failed at index 0: Given db-present startup mode backed by embedded postgres
...
136 Run with RUST_BACKTRACE=full to include source snippets. }.
137 Set SKIP_TEST_CLUSTER=1 to skip.
138 (feature: /home/user/project/backend/tests/features/
139 user_interests_revision_conflicts.feature, scenario:
140 Missing expected revision after preferences exist returns a conflict)
141 Location: backend/tests/user_interests_revision_conflicts_bdd.rs:222
```

The current source has the equivalent step function at
`backend/tests/user_interests_revision_conflicts_bdd.rs:40`; the line `222`
reference in the failure log came from the earlier file shape that produced the
captured output.

### Failure 1 corroborating evidence

The test target still compiles when not asked to run:

```text
1  Finished `test` profile [optimized + debuginfo] target(s) in 7.40s
2  Executable tests/user_interests_revision_conflicts_bdd.rs (target/debug/deps/user_interests_revision_conflicts_bdd-06bb5d5ef669463a)
```

That narrows this specific failure to runtime setup rather than compilation of
the test target or the new interests code.

### Failure 1 suspected diagnosis

Most likely diagnosis: the original failure was caused by a broken `/dev/null`
inside the container at the time the test ran, and embedded Postgres bootstrap
failed while trying to redirect subprocess output there.

Evidence supporting that diagnosis:

- The failure text is explicit: `sh: 1: cannot create /dev/null: Permission
  denied`.
- The stacktrace locations point into `pg-embed-setup-unpriv`, not backend
  application code.
- The panic originates from `handle_cluster_setup_failure()` in
  `cluster_skip.rs:30`, which is only the error escalator, not the underlying
  defect.
- The test executable builds successfully with `--no-run`, showing the test
  code itself is not the immediate blocker.

Important qualification:

- By the time of this investigation, `/dev/null` had been restored to the
  correct character device (`mode=666`, major `1`, minor `3`). That current
  state does not contradict the failure; it means the environment changed after
  the failing run. The log is therefore the strongest evidence of the original
  failure condition.

## Failure 2: `make lint` fails during `cargo clippy` probe

### Failure 2 command

```text
make lint
```

### Failure 2 primary evidence

The original failure happened in `lint-rust`, after `cargo doc` completed and
when `cargo clippy` tried to probe target information:

```text
189 Documenting backend v0.1.0 (/home/user/project/backend)
190     Checking backend v0.1.0 (/home/user/project/backend)
191     Finished `dev` profile [optimized + debuginfo] target(s) in 53.15s
192    Generated /home/user/project/target/doc/architecture_lint/index.html and 7 other files
193 cargo clippy --workspace --all-targets --all-features -- -D warnings
194 error: failed to run `rustc` to learn about target-specific information
195
196 Caused by:
197   process didn't exit successfully:
198     `/root/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin/
199     clippy-driver /root/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin/rustc -
200     --crate-name ___ --print=file-names --crate-type bin --crate-type
201     rlib --crate-type dylib --crate-type cdylib --crate-type staticlib
202     --crate-type proc-macro --print=sysroot --print=split-debuginfo
203     --print=crate-name --print=cfg -Wwarnings` (exit status: 1)
198   --- stdout
199   ___
200   lib___.rlib
201   lib___.so
...
251   unix
252
253   --- stderr
254   error: unknown start of token: `
255    --> <anon>:1:30
256     |
257   1 | warning: missing options for `on_unimplemented` attribute
258     |                              ^
259     |
260   help: Unicode character '`' (Grave Accent) looks like ''' (Single Quote), but it is not
```

The same stderr stream continues with the same pattern:

```text
266   error: unknown start of token: `
267    --> <anon>:1:47
268     |
269   1 | warning: missing options for `on_unimplemented` attribute
270     |                                               ^
...
278   error: unknown start of token: `
279    --> <anon>:7:31
280     |
281   7 |   = help: at least one of the `message`, `note` and `label` options are expected
282     |                               ^
```

### Failure 2 follow-up investigation

The exact probe command was re-run directly, feeding it `/dev/null` on stdin. It
completed successfully and emitted only the expected target metadata:

```text
1  ___
2  lib___.rlib
3  lib___.so
4  lib___.so
5  lib___.a
6  lib___.so
7  /root/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu
8  off
9  packed
10 unpacked
11 ___
12 debug_assertions
...
48 target_os="linux"
49 target_pointer_width="64"
50 target_thread_local
51 target_vendor="unknown"
52 ub_checks
53 unix
```

The command `cargo clippy --workspace --all-targets --all-features -- -D
warnings` was also re-run. This time it did not hit the target-probe
failure at all. Instead, it progressed into ordinary workspace linting and
failed on unrelated, deterministic lint expectations:

```text
11  Checking backend v0.1.0 (/home/user/project/backend)
12 error: this lint expectation is unfulfilled
13  --> backend/tests/pwa_preferences_bdd.rs:4:5
14   |
15 4 |     clippy::type_complexity,
16   |     ^^^^^^^^^^^^^^^^^^^^^^^
...
24 error: this lint expectation is unfulfilled
25  --> backend/tests/pwa_annotations_bdd.rs:4:5
26   |
27 4 |     clippy::type_complexity,
28   |     ^^^^^^^^^^^^^^^^^^^^^^^
```

### Failure 2 suspected diagnosis

Most likely diagnosis: the original `make lint` failure was a transient
toolchain-process or stdin/stderr corruption issue during `cargo clippy`'s
`rustc -` probe, not a stable source-level bug in this repository.

Evidence supporting that diagnosis:

- The failure happened before normal workspace linting began.
- The probe's stdout was complete and plausible target metadata.
- The stderr looked like warning/help text being parsed as anonymous Rust
  source (`<anon>`), which is not consistent with ordinary crate code being
  compiled.
- The exact probe succeeds when run directly later in the same environment.
- A later `cargo clippy` run reaches normal linting and reports unrelated,
  comprehensible source issues instead.

What the available evidence does not establish:

- Whether the root cause was a nightly Rust/Clippy regression, a transient
  cargo-to-clippy plumbing bug, or environment-specific stdin/stderr handling
  corruption in the container.

The strongest safe statement is that the original failure was transient,
toolchain-adjacent, and not reproducible on demand during follow-up.

## Failure 3: `make test` fails in `prepare-pg-worker` with `os error 2`

### Failure 3 command

```text
make test
```

### Failure 3 primary evidence

The recorded failure occurred in `prepare-pg-worker`, which comes from
`Makefile:167` and executes `cargo build -p backend --bin pg_worker`:

```text
1   ./scripts/sync_workspace_members.py
2   RUSTFLAGS="-D warnings" cargo build -p backend --bin pg_worker
...
172 Compiling unicode-bidi v0.3.18
173 error: could not compile `unicode-bidi` (lib)
174
175 Caused by:
176   could not execute process
177     `/root/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin/
178     rustc --crate-name unicode_bidi --edition=2018
179     /root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/
180     unicode-bidi-0.3.18/src/lib.rs --error-format=json
181     --json=diagnostic-rendered-ansi,artifacts,future-incompat
182     --crate-type lib --emit=dep-info,metadata,link -C opt-level=1
183     -C embed-bitcode=no -C debuginfo=1 -C debug-assertions=on
184     --cfg 'feature="default"' --cfg 'feature="hardcoded-data"'
185     --cfg 'feature="std"' --check-cfg 'cfg(docsrs,test)'
186     --check-cfg 'cfg(feature, values("bench_it", "default", "flame",
187     "flame_it", "flamer", "hardcoded-data", "serde", "smallvec", "std",
188     "unstable", "with_serde"))' -C metadata=3b79c94b86f3eb60
189     -C extra-filename=-e56cb86643769919 --out-dir
190     /home/user/project/target/debug/deps -L dependency=/home/user/project/target/debug/deps
191     --cap-lints allow -D warnings` (never executed)
177
178 Caused by:
179   No such file or directory (os error 2)
180 warning: build failed, waiting for other jobs to finish...
181 make: *** [Makefile:167: prepare-pg-worker] Error 101
```

Two points are important here:

- The failing crate is `unicode-bidi`, not backend application code.
- Cargo says the `rustc` process was never executed.

### Failure 3 follow-up investigation

The narrower build command was re-run directly:

```text
RUSTFLAGS="-D warnings" cargo build -p backend --bin pg_worker
```

This later completed successfully:

```text
320 Compiling pg-embed-setup-unpriv v0.5.0
321 Compiling wildside-data v0.1.0 (https://github.com/leynos/wildside-engine.git?rev=894aa38cf0f2ddc870b382880b5db936a761020a#894aa38c)
322 Compiling ortho_config v0.7.0
323 Compiling diesel_migrations v2.2.0
324 Compiling diesel-async v0.5.2
325 Compiling actix-session v0.11.0
326 Compiling actix-ws v0.3.0
327 Compiling mockable v0.3.0
328 Compiling backend v0.1.0 (/home/user/project/backend)
329  Finished `dev` profile [optimized + debuginfo] target(s) in 1m 58s
```

### Failure 3 suspected diagnosis

Most likely diagnosis: the original `os error 2` was another transient
toolchain execution failure in the container, not a persistent absence of the
`rustc` binary or a backend-code regression.

Evidence supporting that diagnosis:

- The exact `rustc` path from the failure log exists when inspected.
- Its dynamic loader and shared-library dependencies resolve via `ldd`.
- Re-running the same `cargo build -p backend --bin pg_worker` path later
  succeeds.
- The original failure happens while compiling a third-party dependency rather
  than project code.

Important limitation:

- `os error 2` on `execve` can also be caused by a missing interpreter or
  loader on an otherwise present binary. Current `ldd` output shows that those
  dependencies were available during follow-up, but it does not prove they were
  available at the exact instant of the original failure.

## Cross-cutting assessment

Across all three failures, the evidence points more strongly to an unstable or
mutating local execution environment than to defects in the `3.5.4`
implementation itself.

Evidence for that assessment:

- The BDD test target compiles, but the runtime fails in embedded Postgres
  bootstrap.
- The original `cargo clippy` probe failure is not reproducible on demand.
- The original `prepare-pg-worker` `os error 2` is not reproducible on demand.
- `/dev/null` was correct by the time of follow-up, implying the environment
  changed between failure capture and investigation.

The following confidence levels are therefore assigned:

- `/dev/null` as the proximate cause of the original BDD failure: high.
- Transient toolchain-process instability as the cause of the original
  `make lint` failure: medium-high.
- Transient toolchain execution instability as the cause of the original
  `make test` `os error 2`: medium.

## What was ruled out

- A compile error in `backend/tests/user_interests_revision_conflicts_bdd.rs`
  causing the BDD failure.
  - Ruled out by the successful `--no-run` build.
- A permanently missing `rustc` or `clippy-driver` binary.
  - Ruled out by file existence, version checks, and successful later runs.
- A stable, deterministic reproduction of the original `cargo clippy` probe
  failure.
  - Ruled out by direct probe success and the later normal clippy failure mode.
- A stable, deterministic reproduction of the original `prepare-pg-worker`
  `os error 2`.
  - Ruled out by the later successful direct `cargo build`.

## Recommended next steps

- Preserve the original failing logs in build artefacts whenever these issues
  recur. The transient failures leave little evidence once the environment
  changes.
- If the `cargo clippy` probe failure recurs, capture:
  - `env`
  - `strace -f -o /tmp/clippy.strace cargo clippy ...`
  - the exact stdout/stderr of the probe command
- If the `prepare-pg-worker` `os error 2` recurs, capture:
  - `strace -f -o /tmp/pg-worker-build.strace cargo build -p backend --bin pg_worker`
  - `ls -l` and `ldd` for the exact `rustc` path named in the failure
  - `mount`, `df`, and any container lifecycle logs
- Add a cheap preflight check for `/dev/null` before embedded Postgres-backed
  tests:
  - verify it is a character device
  - verify it is writable
  - fail fast with a targeted message before invoking `pg-embed-setup-unpriv`
