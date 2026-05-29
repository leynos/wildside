#!/usr/bin/env bash
# Warm the PostgreSQL binary cache used by embedded database tests.
#
# CI invokes this before cargo nextest, and developers may run it locally when
# they need deterministic PostgreSQL bootstrap without repeated downloads. The
# script populates the pg-embed-setup-unpriv cache and can copy from the
# postgresql_embedded Theseus cache when that cache is already present.
#
# Environment:
# - PG_EMBEDDED_VERSION or POSTGRESQL_VERSION: exact PostgreSQL version.
# - PG_BINARY_CACHE_DIR: override the pg-embed-setup-unpriv cache root.
# - XDG_CACHE_HOME: fallback cache root before ~/.cache when set.
# - POSTGRESQL_RELEASES_URL: override the Theseus release repository URL.
set -euo pipefail

CACHE_LOCK_DIR=''
DOWNLOAD_WORK_DIR=''
EXIT_CLEANUP_REGISTERED=false
PREVIOUS_EXIT_HANDLER=''

# Write an informational message to stderr.
#
# Arguments:
#   $* - message text.
#
# Outputs:
#   Writes the formatted log line to stderr.
#
# Returns:
#   0 on success.
log() {
  printf '[pg-embedded-cache] %s\n' "$*" >&2
}

# Write a warning message to stderr.
#
# Arguments:
#   $* - message text.
#
# Outputs:
#   Writes the formatted warning line to stderr.
#
# Returns:
#   0 on success.
warn() {
  printf '[pg-embedded-cache] warning: %s\n' "$*" >&2
}

# Write an error message to stderr and terminate.
#
# Arguments:
#   $* - message text.
#
# Outputs:
#   Writes the formatted error line to stderr.
#
# Returns:
#   Does not return; exits with status 1.
fail() {
  printf '[pg-embedded-cache] error: %s\n' "$*" >&2
  exit 1
}

# Remove the active download work directory when one exists.
#
# Extended detail:
#   The path is tracked globally so the EXIT trap can clean up interrupted
#   downloads without knowing which helper created the directory.
#
# Returns:
#   0 on success.
cleanup_download_work_dir() {
  if [[ -n "$DOWNLOAD_WORK_DIR" ]]; then
    rm -rf "$DOWNLOAD_WORK_DIR"
    DOWNLOAD_WORK_DIR=''
  fi
}

# Resolve and validate the PostgreSQL version.
#
# Extended detail:
#   `PG_EMBEDDED_VERSION` takes precedence over `POSTGRESQL_VERSION`. A leading
#   exact-version marker (`=`) is accepted for compatibility with
#   `postgresql_embedded` and removed before use.
#
# Outputs:
#   Writes the normalized version to stdout.
#
# Returns:
#   0 on success, exits non-zero for non-numeric versions.
normalise_version() {
  local raw_version="${PG_EMBEDDED_VERSION:-${POSTGRESQL_VERSION:-16.10.0}}"
  local version_pattern='^[0-9]+([.][0-9]+)*$'
  raw_version="${raw_version#=}"

  if [[ ! "$raw_version" =~ $version_pattern ]]; then
    fail "expected an exact PostgreSQL version, got '${raw_version}'"
  fi

  printf '%s\n' "$raw_version"
}

# Determine the Theseus platform triple for the current host.
#
# Outputs:
#   Writes the supported platform triple to stdout.
#
# Returns:
#   0 on success, exits non-zero for unsupported platforms.
platform_triple() {
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64)
      printf 'x86_64-unknown-linux-gnu\n'
      ;;
    Linux:aarch64)
      printf 'aarch64-unknown-linux-gnu\n'
      ;;
    Darwin:x86_64)
      printf 'x86_64-apple-darwin\n'
      ;;
    Darwin:arm64)
      printf 'aarch64-apple-darwin\n'
      ;;
    *)
      fail "unsupported platform for PostgreSQL cache warm-up: $(uname -s) $(uname -m)"
      ;;
  esac
}

# Resolve the pg-embed-setup-unpriv binary cache root.
#
# Outputs:
#   Writes the cache directory path to stdout.
#
# Returns:
#   0 on success.
cache_dir() {
  if [[ -n "${PG_BINARY_CACHE_DIR:-}" ]]; then
    printf '%s\n' "$PG_BINARY_CACHE_DIR"
  elif [[ -n "${XDG_CACHE_HOME:-}" ]]; then
    printf '%s/pg-embedded/binaries\n' "$XDG_CACHE_HOME"
  elif [[ -n "${HOME:-}" ]]; then
    printf '%s/.cache/pg-embedded/binaries\n' "$HOME"
  else
    printf '%s/pg-embedded/binaries\n' "${TMPDIR:-/tmp}"
  fi
}

# Resolve the Theseus release repository base URL.
#
# Extended detail:
#   A trailing `/releases` suffix is tolerated for compatibility, but the
#   returned value is the repository URL required by `postgresql_archive`.
#
# Outputs:
#   Writes the release repository URL to stdout.
#
# Returns:
#   0 on success.
release_base_url() {
  local raw_url="${POSTGRESQL_RELEASES_URL:-https://github.com/theseus-rs/postgresql-binaries}"
  raw_url="${raw_url%/}"
  raw_url="${raw_url%/releases}"
  printf '%s\n' "$raw_url"
}

# Check whether a version cache directory is ready for reuse.
#
# Arguments:
#   $1 - version directory to inspect.
#
# Returns:
#   0 when the cache is complete, 1 otherwise.
cache_is_complete() {
  local version_dir="$1"
  [[ -f "${version_dir}/.complete" && -x "${version_dir}/bin/postgres" ]]
}

# Capture an existing EXIT trap so cleanup can compose with it.
#
# Extended detail:
#   Bash reports trap bodies as shell-quoted strings. Evaluate only that
#   already-installed handler so the new cleanup can compose with it. This
#   assumes earlier EXIT handlers came from trusted code; an untrusted
#   preinstalled EXIT trap would make this evaluation unsafe.
#
# Returns:
#   0 on success.
capture_existing_exit_handler() {
  local trap_spec

  trap_spec="$(trap -p EXIT || true)"

  if [[ -z "$trap_spec" ]]; then
    return
  fi

  trap_spec="${trap_spec#trap -- }"
  trap_spec="${trap_spec% EXIT}"

  eval "PREVIOUS_EXIT_HANDLER=${trap_spec}"
}

# Run registered cleanup actions before process exit.
#
# Extended detail:
#   Preserves the original exit status while deleting temporary downloads,
#   releasing the lock directory, and invoking any previously registered EXIT
#   trap.
#
# Returns:
#   Exits with the original process status.
run_exit_cleanup() {
  local status=$?

  cleanup_download_work_dir

  if [[ -n "$CACHE_LOCK_DIR" ]]; then
    rm -rf "$CACHE_LOCK_DIR"
  fi

  if [[ -n "$PREVIOUS_EXIT_HANDLER" ]]; then
    eval "$PREVIOUS_EXIT_HANDLER"
  fi

  exit "$status"
}

# Register the EXIT cleanup trap once.
#
# Returns:
#   0 on success.
register_exit_cleanup() {
  if [[ "$EXIT_CLEANUP_REGISTERED" == true ]]; then
    return
  fi

  capture_existing_exit_handler
  trap run_exit_cleanup EXIT
  EXIT_CLEANUP_REGISTERED=true
}

# Read the PID that owns a cache lock directory.
#
# Arguments:
#   $1 - lock directory containing a `pid` file.
#
# Outputs:
#   Writes the PID to stdout.
#
# Returns:
#   0 when a numeric PID is readable, 1 otherwise.
lock_owner_pid() {
  local lock_dir="$1"
  local pid

  if [[ ! -r "${lock_dir}/pid" ]]; then
    return 1
  fi

  if ! IFS= read -r pid <"${lock_dir}/pid"; then
    return 1
  fi

  if [[ ! "$pid" =~ ^[0-9]+$ ]]; then
    return 1
  fi

  printf '%s\n' "$pid"
}

# Remove a stale cache lock whose owning process is gone.
#
# Arguments:
#   $1 - lock directory to inspect and remove.
#
# Outputs:
#   Writes a warning to stderr when a stale lock is removed.
#
# Returns:
#   0 when the stale lock is removed, 1 when the lock is active or invalid.
remove_stale_cache_lock() {
  local lock_dir="$1"
  local owner_pid
  local current_pid

  if ! owner_pid="$(lock_owner_pid "$lock_dir")"; then
    return 1
  fi

  if kill -0 "$owner_pid" 2>/dev/null; then
    return 1
  fi

  if ! current_pid="$(lock_owner_pid "$lock_dir")"; then
    return 1
  fi

  if [[ "$current_pid" != "$owner_pid" ]]; then
    return 1
  fi

  warn "removing stale PostgreSQL cache lock at ${lock_dir} for pid ${owner_pid}"
  rm -f "${lock_dir}/pid"
  rmdir "$lock_dir" 2>/dev/null
}

# Acquire the process-local cache lock directory.
#
# Extended detail:
#   Lock contention is handled by polling the directory and removing only
#   verified stale locks. The loop emits a wait message once and then every
#   thirty seconds.
#
# Arguments:
#   $1 - cache root directory.
#
# Outputs:
#   Writes wait messages and stale-lock warnings to stderr.
#
# Returns:
#   0 on success, exits non-zero on timeout or lock-owner write failure.
acquire_cache_lock() {
  local root_dir="$1"
  local lock_dir="${root_dir}/.warm-pg-embedded-cache.lock"
  local has_logged_wait=false
  local waited_seconds=0

  while ! mkdir "$lock_dir" 2>/dev/null; do
    if remove_stale_cache_lock "$lock_dir"; then
      continue
    fi

    if ((waited_seconds >= 600)); then
      fail "timed out waiting for PostgreSQL cache lock at ${lock_dir}"
    fi

    if [[ "$has_logged_wait" == false ]]; then
      log "waiting for cache lock at ${lock_dir} (pid $$, ${waited_seconds}s elapsed)"
      has_logged_wait=true
    elif ((waited_seconds > 0 && waited_seconds % 30 == 0)); then
      log "still waiting for cache lock at ${lock_dir} (${waited_seconds}s elapsed)"
    fi

    sleep 1
    waited_seconds=$((waited_seconds + 1))
  done

  if ! printf '%s\n' "$$" >"${lock_dir}/pid"; then
    rm -rf "$lock_dir"
    fail "failed to record PostgreSQL cache lock owner at ${lock_dir}"
  fi

  CACHE_LOCK_DIR="$lock_dir"
  register_exit_cleanup
}

# Atomically install a prepared cache directory.
#
# Extended detail:
#   The previous version directory is moved aside first and restored when the
#   final install move fails.
#
# Arguments:
#   $1 - prepared directory to install.
#   $2 - final version directory.
#
# Outputs:
#   Writes rollback warnings or install errors to stderr.
#
# Returns:
#   0 on success, exits non-zero on install failure.
install_cache_dir() {
  local prepared_dir="$1"
  local version_dir="$2"
  local previous_dir
  local rollback_error
  local rollback_status

  previous_dir="$(mktemp -d "${version_dir}.previous.XXXXXX")"
  rmdir "$previous_dir"

  if [[ -e "$version_dir" ]]; then
    if ! mv "$version_dir" "$previous_dir"; then
      rm -rf "$prepared_dir" "$previous_dir"
      fail "failed to move existing PostgreSQL cache at ${version_dir}"
    fi
  fi

  if mv "$prepared_dir" "$version_dir"; then
    rm -rf "$previous_dir"
    return
  fi

  if [[ -e "$previous_dir" && ! -e "$version_dir" ]]; then
    rollback_status=0
    rollback_error="$(mv "$previous_dir" "$version_dir" 2>&1)" || rollback_status=$?
    if ((rollback_status != 0)); then
      warn "failed to restore PostgreSQL cache from ${previous_dir} to ${version_dir}: ${rollback_error} (exit ${rollback_status})"
    fi
  fi

  rm -rf "$prepared_dir"
  fail "failed to install PostgreSQL cache at ${version_dir}"
}

# Populate the cache from postgresql_embedded's Theseus installation cache.
#
# Arguments:
#   $1 - PostgreSQL version.
#   $2 - pg-embed-setup-unpriv version directory.
#
# Outputs:
#   Writes copy progress to stderr.
#
# Returns:
#   0 when copied, 1 when no reusable Theseus cache exists, exits non-zero on
#   copy or completion-marker failures.
populate_from_theseus_cache() {
  local version="$1"
  local version_dir="$2"
  local prepared_dir
  local source_dir

  if [[ -z "${HOME:-}" ]]; then
    return 1
  fi

  source_dir="${HOME}/.theseus/postgresql/${version}"

  if [[ ! -x "${source_dir}/bin/postgres" ]]; then
    return 1
  fi

  log "copying PostgreSQL ${version} from ${source_dir}"
  prepared_dir="$(mktemp -d "${version_dir}.tmp.XXXXXX")"
  if ! cp -a "${source_dir}/." "$prepared_dir/"; then
    rm -rf "$prepared_dir"
    fail "failed to copy PostgreSQL cache from ${source_dir}"
  fi

  if ! touch "${prepared_dir}/.complete"; then
    rm -rf "$prepared_dir"
    fail "failed to mark copied PostgreSQL cache as complete"
  fi

  install_cache_dir "$prepared_dir" "$version_dir"
}

# Verify a downloaded archive against its SHA-256 sidecar.
#
# Arguments:
#   $1 - download work directory.
#   $2 - archive asset name.
#   $3 - platform triple used to select checksum tooling.
#
# Outputs:
#   Writes checksum failure details to stderr.
#
# Returns:
#   0 on success, exits non-zero on checksum failure.
verify_checksum() {
  local work_dir="$1"
  local asset="$2"
  local triple="$3"
  local checksum_output
  local checksum_status=0

  if [[ "$triple" == *-apple-darwin ]]; then
    checksum_output="$(cd "$work_dir" && shasum -a 256 -c "${asset}.sha256" 2>&1)" || checksum_status=$?
  else
    checksum_output="$(cd "$work_dir" && sha256sum -c "${asset}.sha256" 2>&1)" || checksum_status=$?
  fi

  if ((checksum_status != 0)); then
    fail "checksum verification failed for ${asset} using ${asset}.sha256 (exit ${checksum_status}); output: ${checksum_output}. The .sha256 may be malformed or mismatched."
  fi
}

# Download, verify, extract, and install a PostgreSQL archive.
#
# Arguments:
#   $1 - PostgreSQL version.
#   $2 - final version directory.
#   $3 - platform triple.
#   $4 - release repository base URL.
#
# Outputs:
#   Writes download progress and failure diagnostics to stderr.
#
# Returns:
#   0 on success, exits non-zero on download, verification, extraction, or
#   install failure.
download_and_extract() {
  local version="$1"
  local version_dir="$2"
  local triple="$3"
  local base_url="$4"

  local asset="postgresql-${version}-${triple}.tar.gz"
  local work_dir
  work_dir="$(mktemp -d)"
  DOWNLOAD_WORK_DIR="$work_dir"

  log "downloading ${asset}"
  local curl_status=0
  curl --fail --location --retry 5 --retry-all-errors \
    --connect-timeout 30 --max-time 600 --speed-time 60 --speed-limit 1024 \
    --output "${work_dir}/${asset}" \
    "${base_url}/releases/download/${version}/${asset}" || curl_status=$?
  if ((curl_status != 0)); then
    fail "download failed for asset '${asset}' from '${base_url}/releases/download/${version}/${asset}' (curl exit ${curl_status}; connect-timeout 30s, max-time 600s, speed-limit 1024 B/s); cache root: ${version_dir%/*}"
  fi

  curl_status=0
  curl --fail --location --retry 5 --retry-all-errors \
    --connect-timeout 30 --max-time 600 --speed-time 60 --speed-limit 1024 \
    --output "${work_dir}/${asset}.sha256" \
    "${base_url}/releases/download/${version}/${asset}.sha256" || curl_status=$?
  if ((curl_status != 0)); then
    fail "download failed for asset '${asset}.sha256' from '${base_url}/releases/download/${version}/${asset}.sha256' (curl exit ${curl_status}; connect-timeout 30s, max-time 600s, speed-limit 1024 B/s); cache root: ${version_dir%/*}"
  fi

  verify_checksum "$work_dir" "$asset" "$triple"

  local prepared_dir
  prepared_dir="$(mktemp -d "${version_dir}.tmp.XXXXXX")"
  if ! tar -xzf "${work_dir}/${asset}" -C "$prepared_dir" --strip-components=1; then
    rm -rf "$prepared_dir"
    fail "failed to extract PostgreSQL archive"
  fi

  if [[ ! -x "${prepared_dir}/bin/postgres" ]]; then
    rm -rf "$prepared_dir"
    fail "archive did not contain bin/postgres"
  fi

  if ! touch "${prepared_dir}/.complete"; then
    rm -rf "$prepared_dir"
    fail "failed to mark downloaded PostgreSQL cache as complete"
  fi

  install_cache_dir "$prepared_dir" "$version_dir"
  cleanup_download_work_dir
}

# Warm the embedded PostgreSQL binary cache.
#
# Outputs:
#   Writes cache status and completion diagnostics to stderr.
#
# Returns:
#   0 on success, exits non-zero on cache preparation failure.
main() {
  local version
  local root_dir
  local triple
  local version_dir

  version="$(normalise_version)"
  root_dir="$(cache_dir)"
  triple="$(platform_triple)"
  version_dir="${root_dir}/${version}"
  mkdir -p "$root_dir"
  acquire_cache_lock "$root_dir"

  if cache_is_complete "$version_dir"; then
    log "cache hit for PostgreSQL ${version} at ${version_dir}"
    log "completed: platform=${triple} version=${version} cache=${version_dir}"
    return
  fi

  if populate_from_theseus_cache "$version" "$version_dir"; then
    log "cache warmed for PostgreSQL ${version} at ${version_dir}"
    log "completed: platform=${triple} version=${version} cache=${version_dir}"
    return
  fi

  download_and_extract "$version" "$version_dir" "$triple" "$(release_base_url)"
  log "cache warmed for PostgreSQL ${version} at ${version_dir}"
  log "completed: platform=${triple} version=${version} cache=${version_dir}"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
