# Phase 1 session signing key rotation

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / Big Picture

Enable zero-downtime rotation of session signing keys through operational
procedures and tooling. The backend uses `actix-session` with
`CookieSessionStore` for stateless, cookie-based sessions signed with a secret
key. Rotation relies on rolling deployment overlap rather than in-app dual-key
validation. Success is observable when:

- The server logs the active key fingerprint on startup for operational
  visibility.
- Helm chart supports session secret volume mounts with checksum annotations to
  trigger rolling restarts on rotation.
- A rotation script automates key generation, secret update, and rollout
  monitoring.
- A runbook documents the complete rotation procedure including rollback.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd` v0.2.0) cover the
  fingerprinting logic.
- `docs/wildside-backend-architecture.md` records the rotation procedure and
  fingerprinting design.
- `docs/backend-roadmap.md` marks the signing key rotation task as done.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Progress

- [x] Draft ExecPlan for session signing key rotation.
- [x] Add sha2 and hex dependencies to backend/Cargo.toml.
- [x] Create fingerprint module with unit tests.
- [x] Update session_config.rs to export fingerprint and add to SessionSettings.
- [x] Update main.rs to log fingerprint on startup.
- [x] Update Helm chart with session secret volume mounts and checksum.
- [x] Create rotation script.
- [x] Create runbook documentation.
- [x] Update architecture documentation.
- [x] Create BDD feature file and test implementation.
- [x] Mark roadmap task as complete.
- [x] Run quality gates.

## Surprises & Discoveries

- The project scripting standard prefers Python with `uv` over shell scripts, so
  the rotation script was created as `scripts/rotate_session_key.py` using
  `plumbum` for command execution rather than a Bash script.
- The runbooks directory did not exist and was created as part of this work.

## Decision Log

- Decision: Use rolling deployment overlap rather than in-app dual-key
  validation.
  Rationale: The architecture document prescribes this approach. Sessions have
  a 2-hour TTL, so the overlap window during rolling deployment allows existing
  sessions to remain valid on old pods while new pods use the new key. This
  avoids custom session store wrappers and keeps the implementation simple.
  Date/Author: 2025-12-21 / Claude Code.

- Decision: Use SHA-256 fingerprint truncated to 16 hex characters.
  Rationale: Sufficient for visual distinction in logs and runbooks. Not
  security-sensitive since it's for operational identification, not
  authentication.
  Date/Author: 2025-12-21 / Claude Code.

- Decision: Use volume-mounted secrets rather than environment variables.
  Rationale: Matches existing `SESSION_KEY_FILE` pattern and allows binary key
  material without base64 encoding complexity.
  Date/Author: 2025-12-21 / Claude Code.

## Outcomes & Retrospective

Implementation complete. All acceptance criteria met:

1. Key fingerprinting implemented in
   `backend/src/inbound/http/session_config/fingerprint.rs` with unit tests
   covering determinism, format, and distinctness.

2. Kubernetes integration added to Helm chart with session secret volume mounts
   (disabled by default) and checksum annotation for rolling restarts.

3. Rotation tooling delivered:
   - `scripts/rotate_session_key.py` automates key generation, secret update,
     and rollout monitoring
   - `docs/runbooks/session-key-rotation.md` documents complete rotation
     lifecycle

4. Testing and documentation:
   - Unit tests: 5 tests in fingerprint module
   - BDD tests: 4 scenarios in `session_key_fingerprint.feature`
   - Architecture documentation expanded with rotation guidance
   - Roadmap task marked complete

5. All quality gates pass:
   - `make check-fmt`: PASS
   - `make lint`: PASS
   - `make test`: 182 tests passed, 1 skipped

Date completed: 2025-12-21.

## Context and Orientation

Key locations (repository-relative):

- `backend/src/inbound/http/session_config.rs`: session configuration parsing
  and validation.
- `backend/src/inbound/http/session_config/test_utils.rs`: test utilities for
  session configuration.
- `backend/src/inbound/http/session_config/tests.rs`: unit tests for session
  configuration.
- `backend/src/server/mod.rs`: server bootstrap and session middleware wiring.
- `backend/src/main.rs`: application entry point.
- `deploy/charts/wildside/templates/deployment.yaml`: Kubernetes deployment.
- `deploy/charts/wildside/values.yaml`: Helm chart values.
- `docs/wildside-backend-architecture.md`: architecture documentation.
- `docs/backend-roadmap.md`: Phase 1 checklist entry to mark done.

Terminology (plain-language):

- *Fingerprint*: A truncated SHA-256 hash of the signing key material, used to
  identify which key is active without exposing the key itself.
- *Rolling deployment*: Kubernetes deployment strategy where new pods are
  started before old pods are terminated, ensuring continuous availability.
- *Session TTL*: The 2-hour time-to-live for session cookies before they
  expire.

## Plan of Work

1. Add dependencies (`sha2`, `hex`) to `backend/Cargo.toml` for fingerprint
   computation.

2. Create `backend/src/inbound/http/session_config/fingerprint.rs`:
   - Implement `key_fingerprint(&Key) -> String` function.
   - SHA-256 hash of the key's signing material.
   - Truncate to first 8 bytes and encode as 16-character hex string.
   - Add unit tests for determinism, format, and distinctness.

3. Update `backend/src/inbound/http/session_config.rs`:
   - Add `pub mod fingerprint;` declaration.
   - Add `fingerprint: String` field to `SessionSettings`.
   - Compute fingerprint in `session_settings_from_env`.

4. Update `backend/src/main.rs`:
   - After loading session settings, log fingerprint at INFO level.
   - Format: `session signing key loaded fingerprint=<hex>`.

5. Update `deploy/charts/wildside/templates/deployment.yaml`:
   - Add `volumes` section for session secret.
   - Add `volumeMounts` to container for `/var/run/secrets/session_key`.
   - Add checksum annotation for secret to trigger rolling restart.

6. Update `deploy/charts/wildside/values.yaml`:
   - Add `sessionSecret.enabled` (default: `false`).
   - Add `sessionSecret.name` (default: `wildside-session-key`).
   - Add `sessionSecret.keyName` (default: `session_key`).
   - Add `sessionSecret.mountPath` (default: `/var/run/secrets`).

7. Create `scripts/rotate_session_key.py`:
   - Parse namespace and secret name arguments using argparse.
   - Generate new 64-byte key using Python's secrets module.
   - Compute fingerprints of old and new keys (matching backend's HKDF
     derivation).
   - Update Kubernetes Secret with `kubectl patch`.
   - Trigger rolling restart with `kubectl rollout restart`.
   - Monitor rollout status with `kubectl rollout status`.
   - Output summary with fingerprints for runbook logging.

8. Create `docs/runbooks/session-key-rotation.md`:
   - Pre-rotation checklist.
   - Step-by-step rotation procedure.
   - Post-rotation validation.
   - Rollback procedure.
   - Troubleshooting guide.
   - Rotation schedule recommendations.

9. Update `docs/wildside-backend-architecture.md`:
   - Expand "Session Configuration and Rotation" section.
   - Document fingerprinting for operational visibility.
   - Reference runbook.
   - Clarify rolling overlap requirements (≥2 replicas).

10. Create `backend/tests/features/session_key_fingerprint.feature`:
    - Scenario: Key fingerprint is deterministic.
    - Scenario: Different keys produce different fingerprints.
    - Scenario: Fingerprint is valid hex format.

11. Create `backend/tests/session_key_fingerprint_bdd.rs`:
    - Implement BDD step definitions.
    - Follow patterns from `session_config_bdd.rs`.

12. Mark the roadmap task as done in `docs/backend-roadmap.md`.

13. Run quality gates:
    - `make check-fmt`
    - `make lint`
    - `make test`
    - `make markdownlint`

## Concrete Steps

Run these commands from the repository root. Use a 300-second timeout by
default and capture output with `tee` so logs are preserved.

1. After code changes:

   ```bash
   set -o pipefail
   timeout 300 make fmt 2>&1 | tee /tmp/wildside-fmt.log
   ```

2. Check formatting:

   ```bash
   set -o pipefail
   timeout 300 make check-fmt 2>&1 | tee /tmp/wildside-check-fmt.log
   ```

3. Lint:

   ```bash
   set -o pipefail
   timeout 300 make lint 2>&1 | tee /tmp/wildside-lint.log
   ```

4. Test (may take longer than 300s):

   ```bash
   set -o pipefail
   timeout 600 make test 2>&1 | tee /tmp/wildside-test.log
   ```

5. Markdown lint (if documentation updated):

   ```bash
   set -o pipefail
   timeout 300 make markdownlint 2>&1 | tee /tmp/wildside-markdownlint.log
   ```

If running tests locally without elevated permissions for the Postgres worker,
use the helper described in `docs/pg-embed-setup-unpriv-users-guide.md`:

```bash
set -o pipefail
PG_WORKER_PATH=/tmp/pg_worker timeout 600 make test 2>&1 \
    | tee /tmp/wildside-test.log
```

## Validation and Acceptance

Acceptance criteria:

1. Key fingerprinting:
   - Fingerprint is logged on server startup at INFO level.
   - Fingerprint is deterministic for the same key.
   - Different keys produce different fingerprints.
   - Fingerprint format is 16 hexadecimal characters.

2. Kubernetes integration:
   - Helm chart supports optional session secret volume mount.
   - Checksum annotation triggers rolling restart when secret changes.
   - Volume mount places key file at expected path.

3. Rotation tooling:
   - Script generates cryptographically secure 64-byte key.
   - Script updates Kubernetes Secret and triggers rollout.
   - Script outputs fingerprints for audit trail.
   - Runbook covers complete rotation lifecycle.

4. Testing and documentation:
   - Unit tests cover fingerprint determinism and format.
   - BDD tests cover configuration scenarios.
   - Architecture documentation expanded with rotation guidance.
   - Roadmap task marked complete.
   - All quality gates pass.

## Idempotence and Recovery

- The fingerprint module is pure and testable.
- Helm chart changes are additive and backwards-compatible (disabled by
  default).
- Rotation script is safe to re-run; repeated rotation simply updates the key
  again.
- If a command fails, fix the issue and re-run only the failed command.

## Artifacts and Notes

Keep log files created by the `tee` commands until the work is complete:

- `/tmp/wildside-check-fmt.log`
- `/tmp/wildside-lint.log`
- `/tmp/wildside-test.log`
- `/tmp/wildside-markdownlint.log`

## Interfaces and Dependencies

New dependencies in `backend/Cargo.toml`:

```toml
sha2 = "0.10"
hex = "0.4"
```

New function in `backend/src/inbound/http/session_config/fingerprint.rs`:

```rust
/// Generate a truncated SHA-256 fingerprint of the key's signing material.
///
/// Returns the first 8 bytes as a hex string (16 characters).
pub fn key_fingerprint(key: &actix_web::cookie::Key) -> String
```

Updated struct in `backend/src/inbound/http/session_config.rs`:

```rust
pub struct SessionSettings {
    pub key: Key,
    pub cookie_secure: bool,
    pub same_site: SameSite,
    pub fingerprint: String,  // NEW
}
```
