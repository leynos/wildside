# Phase 1 session configuration toggles

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / Big Picture

Strengthen session configuration so production startup fails fast when session
secrets or configuration toggles are missing or invalid. The API must still use
session middleware, but configuration parsing should be explicit, testable, and
predictable. Success is observable when:

- Starting the server in a release build without valid session configuration
  exits with a clear error.
- Debug builds continue to work with safe defaults, while logging that defaults
  were applied.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd` v0.2.0) cover happy
  and unhappy paths for the toggles and key length rules.
- `docs/wildside-backend-architecture.md` records the decision on strict
  toggle enforcement.
- `docs/backend-roadmap.md` marks the toggle enforcement task as done.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Progress

- [x] (2025-12-20) Draft ExecPlan for session configuration toggles.
- [x] (2025-12-20) Audited session configuration parsing and tests.
- [x] (2025-12-20) Added shared session configuration module with strict
  validation.
- [x] (2025-12-20) Updated server bootstrap to use shared session
  configuration.
- [x] (2025-12-20) Added unit tests with `rstest` for debug/release paths and
  edge cases.
- [x] (2025-12-20) Added behavioural tests with `rstest-bdd` and a new Gherkin
  feature.
- [x] (2025-12-20) Updated architecture documentation and roadmap status.
- [x] (2025-12-20) Ran `make markdownlint`, `make nixie`, `make fmt`,
  `make check-fmt`, `make lint`, and `make test` (with an extended timeout).

## Surprises & Discoveries

- Observation: `make nixie` required installing the Mermaid puppeteer
  dependency via `make deps` before it would pass. Evidence: `make nixie`
  failed before `make deps`, then passed after.
- Observation: `make test` exceeded the default 300-second timeout and needed
  a rerun with `timeout 900`. Evidence: initial run exited with code 124; rerun
  succeeded.

## Decision Log

- Decision: Define a local `SessionEnv` trait with a `DefaultEnv` implementation
  for production reads, while tests supply a `MockEnv`. Rationale: Keeps tests
  deterministic and avoids a runtime dependency on the `mockable` crate by
  limiting it to dev-only usage. Date/Author: 2025-12-20 / Codex CLI.
- Decision: Move unit tests into
  `backend/src/inbound/http/session_config/tests.rs`. Rationale: Keeps the main
  module under the 400-line limit while preserving proximity to the
  implementation. Date/Author: 2025-12-20 / Codex CLI.
- Decision: Include `min_len` in `SessionConfigError::KeyTooShort`.
  Rationale: Provides actionable error details without additional logging.
  Date/Author: 2025-12-20 / Codex CLI.

## Outcomes & Retrospective

Session configuration toggles are now strictly enforced in release builds and
relaxed with warnings in debug builds. A dedicated module encapsulates parsing
and validation, unit and behavioural tests cover happy/unhappy paths, and
documentation plus the roadmap were updated. All quality gates succeeded,
including extended tests for Postgres-backed scenarios.

## Context and Orientation

Key locations (repository-relative):

- `backend/src/main.rs`: current session configuration parsing helpers
  (`load_session_key`, `cookie_secure_from_env`, `same_site_from_env`).
- `backend/src/server/mod.rs`: session middleware wiring in `build_app`.
- `backend/src/server/config.rs`: `ServerConfig` builder used by the server.
- `backend/src/inbound/http/session.rs`: session wrapper used by handlers.
- `backend/src/inbound/http/session_config.rs`: shared configuration parser.
- `backend/src/inbound/http/session_config/tests.rs`: unit tests for session
  configuration.
- `backend/tests/`: behavioural tests using `rstest-bdd`.
- `backend/tests/features/`: existing Gherkin feature files.
- `docs/wildside-backend-architecture.md`: session configuration decision
  section.
- `docs/backend-roadmap.md`: Phase 1 checklist entry to mark done.
- `docs/pg-embed-setup-unpriv-users-guide.md`: Postgres bootstrap guidance for
  local tests.

Terminology (plain-language):

- *Toggle*: an environment variable used as a boolean or selector to configure
  behaviour (e.g. `SESSION_COOKIE_SECURE`).
- *Release build*: a non-debug build where `cfg!(debug_assertions)` is false.
- *Ephemeral key*: a randomly generated session signing key used only in
  development when a persistent key file is missing.

## Plan of Work

1. Audit the existing parsing logic in `backend/src/main.rs` and confirm how
   `SESSION_ALLOW_EPHEMERAL`, `SESSION_COOKIE_SECURE`, and `SESSION_SAMESITE`
   are currently handled. Identify where release builds already fail fast and
   where defaults are still silently applied.

2. Introduce a shared session configuration module under
   `backend/src/inbound/http/session_config.rs` (add it to
   `backend/src/inbound/http/mod.rs`). This module will:

   - Use a local `SessionEnv` trait to read environment variables so tests can
     avoid touching process-wide state.
   - Parse `SESSION_COOKIE_SECURE` and `SESSION_SAMESITE` with strict validation
     in release mode; missing or invalid values must return a structured error.
   - Parse `SESSION_ALLOW_EPHEMERAL` as an explicit boolean toggle; in release
     mode it must be set and disallow `1` (ephemeral keys remain a debug-only
     escape hatch).
   - Enforce `SESSION_SAMESITE=None` only when `SESSION_COOKIE_SECURE` is true.
   - Load the key from `SESSION_KEY_FILE` (defaulting to
     `/var/run/secrets/session_key`), fail fast on missing keys in release
     mode, and require at least 64 bytes in release builds. Zeroize raw key
     bytes after deriving `Key`.
   - Provide a `SessionSettings` struct with `Key`, `cookie_secure`, and
     `same_site` fields for use by the server bootstrap.

3. Update `backend/src/main.rs` to replace the inline helper functions with the
   new shared module. Use `DefaultEnv` (backed by the process environment) and
   a `BuildMode` derived from `cfg!(debug_assertions)` to select strict or
   relaxed behaviour. Keep warnings in debug builds when defaults are used.

4. Add unit tests (`rstest`) in the new session configuration module. Include
   fixtures for `MockEnv`, a temporary key file path, and helper builders for
   debug vs release modes. Cover:

   - Release mode: missing or invalid `SESSION_COOKIE_SECURE` errors.
   - Release mode: missing or invalid `SESSION_SAMESITE` errors.
   - Release mode: `SESSION_ALLOW_EPHEMERAL` missing or set to `1` errors.
   - Release mode: key file missing or shorter than 64 bytes errors.
   - Happy path: release mode succeeds with explicit toggles, `SameSite` and
     cookie-secure values match, and the derived key is accepted.
   - Edge case: `SESSION_SAMESITE=None` with `SESSION_COOKIE_SECURE=0` rejects.

5. Add behavioural tests (`rstest-bdd` v0.2.0) in
   `backend/tests/session_config_bdd.rs` with a new feature file at
   `backend/tests/features/session_config.feature`. Define scenarios that:

   - Assert release mode fails fast when toggles are missing.
   - Assert release mode accepts explicit, valid configuration with a 64-byte
     key file.
   - Assert `SameSite=None` with `SESSION_COOKIE_SECURE=0` fails.

   Use fixtures for `MockEnv`, a temp key file, and a shared world struct that
   stores the last `Result<SessionSettings, SessionConfigError>` so `Then`
   steps can assert on errors or computed values.

6. Update `docs/wildside-backend-architecture.md` in the “Session
   Configuration and Rotation” section to document the strict release-mode
   enforcement for these toggles, including the rule that
   `SESSION_ALLOW_EPHEMERAL` is a debug-only escape hatch.

7. Mark the roadmap task as done in `docs/backend-roadmap.md` under Phase 1 →
   Session lifecycle hardening.

8. Run documentation and code quality gates, then fix any failures:

   - `make markdownlint` (docs changes)
   - `make nixie` (Mermaid validation)
   - `make check-fmt`
   - `make lint`
   - `make test`

## Concrete Steps

Run these commands from the repository root. Use a 300-second timeout by
default and capture output with `tee` so logs are preserved.

1. If documentation is updated:

    set -o pipefail
    timeout 300 make markdownlint 2>&1 | tee /tmp/wildside-markdownlint.log
    timeout 300 make nixie 2>&1 | tee /tmp/wildside-nixie.log

   If `make nixie` fails because the Mermaid puppeteer dependency is missing,
   run:

    set -o pipefail
    timeout 300 make deps 2>&1 | tee /tmp/wildside-deps.log
    timeout 300 make nixie 2>&1 | tee /tmp/wildside-nixie.log

2. Format if needed:

    set -o pipefail
    timeout 300 make fmt 2>&1 | tee /tmp/wildside-fmt.log

3. Check formatting:

    set -o pipefail
    timeout 300 make check-fmt 2>&1 | tee /tmp/wildside-check-fmt.log

4. Lint:

    set -o pipefail
    timeout 300 make lint 2>&1 | tee /tmp/wildside-lint.log

5. Test (may take longer than 300s; if it times out, rerun with a higher
   timeout and keep the log):

    set -o pipefail
    timeout 300 make test 2>&1 | tee /tmp/wildside-test.log

   If the 300-second timeout is exceeded, rerun with a larger window:

    set -o pipefail
    timeout 900 make test 2>&1 | tee /tmp/wildside-make-test-20251220T0000.log

If running tests locally without elevated permissions for the Postgres worker,
use the helper described in `docs/pg-embed-setup-unpriv-users-guide.md`, for
example:

    set -o pipefail
    PG_WORKER_PATH=/tmp/pg_worker timeout 300 make test 2>&1 \
        | tee /tmp/wildside-test.log

Always check the exit status of each command and inspect the log if a command
fails.

## Validation and Acceptance

Acceptance criteria:

1. Release-mode configuration validation:

   - Missing or invalid `SESSION_COOKIE_SECURE` fails fast with a clear error.
   - Missing or invalid `SESSION_SAMESITE` fails fast with a clear error.
   - `SESSION_ALLOW_EPHEMERAL` is explicitly required in release mode and may
     not be set to `1`.
   - Missing session key file or keys shorter than 64 bytes fail fast.
   - `SESSION_SAMESITE=None` requires `SESSION_COOKIE_SECURE=1`.

2. Debug-mode configuration validation:

   - Defaults are applied when toggles are missing, and warnings are emitted.
   - `SESSION_ALLOW_EPHEMERAL=1` permits an ephemeral key when the key file is
     missing.

3. Testing and tooling:

   - Unit tests (`rstest`) cover the scenarios above.
   - Behavioural tests (`rstest-bdd` v0.2.0) cover success and failure flows.
   - `make check-fmt`, `make lint`, and `make test` complete successfully.
   - Documentation updates are reflected in
     `docs/wildside-backend-architecture.md` and
     `docs/backend-roadmap.md`.

## Idempotence and Recovery

- The steps are safe to re-run. Configuration parsing is pure and testable.
- If a command fails, fix the issue and re-run only the failed command.
- If `make test` fails because the Postgres worker path is unwritable, rerun
  with `PG_WORKER_PATH` set to a user-writable directory as shown above.

## Artifacts and Notes

Keep log files created by the `tee` commands until the work is complete, then
remove them if no longer needed. Key logs:

- `/tmp/wildside-check-fmt.log`
- `/tmp/wildside-lint.log`
- `/tmp/wildside-test.log`
- `/tmp/wildside-make-test-20251220T0000.log`

## Interfaces and Dependencies

Implement these interfaces in `backend/src/inbound/http/session_config.rs` and
expose the module via `backend::inbound::http`:

    pub enum BuildMode {
        Debug,
        Release,
    }

    pub struct SessionSettings {
        pub key: actix_web::cookie::Key,
        pub cookie_secure: bool,
        pub same_site: actix_web::cookie::SameSite,
    }

    pub fn session_settings_from_env<E: SessionEnv>(
        env: &E,
        mode: BuildMode,
    ) -> Result<SessionSettings, SessionConfigError>

    pub trait SessionEnv {
        fn string(&self, name: &str) -> Option<String>;
    }

    pub struct DefaultEnv;

    #[derive(thiserror::Error, Debug)]
    pub enum SessionConfigError {
        MissingEnv { name: &'static str },
        InvalidEnv {
            name: &'static str,
            value: String,
            expected: &'static str,
        },
        KeyRead { path: std::path::PathBuf, source: std::io::Error },
        KeyTooShort { path: std::path::PathBuf, length: usize, min_len: usize },
        InsecureSameSiteNone,
        EphemeralNotAllowed,
    }

If `mockable` is not yet a dev-dependency, add it to `backend/Cargo.toml` with
a caret requirement (for example
`mockable = { version = "0.3", features = ["mock"] }`) so tests can use
`MockEnv`.

In `backend/src/main.rs`, replace the inline parsing helpers with a call to the
new module using `DefaultEnv` and a `BuildMode` derived from
`cfg!(debug_assertions)`.

## Revision note (2025-12-20)

This ExecPlan was updated to reflect completed implementation work, test runs,
and the switch to `DefaultEnv` for production reads. Progress, decisions,
artifacts, and interfaces now match the shipped code and the remaining work is
fully complete. Updated the decision log and interface guidance to describe the
local `SessionEnv` trait and `DefaultEnv`, reflecting the shift of `mockable`
to a dev-only dependency.
