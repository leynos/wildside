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

log() {
  printf '[pg-embedded-cache] %s\n' "$*" >&2
}

warn() {
  printf '[pg-embedded-cache] warning: %s\n' "$*" >&2
}

fail() {
  printf '[pg-embedded-cache] error: %s\n' "$*" >&2
  exit 1
}

cleanup_download_work_dir() {
  if [[ -n "$DOWNLOAD_WORK_DIR" ]]; then
    rm -rf "$DOWNLOAD_WORK_DIR"
    DOWNLOAD_WORK_DIR=''
  fi
}

normalise_version() {
  local raw_version="${PG_EMBEDDED_VERSION:-${POSTGRESQL_VERSION:-16.10.0}}"
  local version_pattern='^[0-9]+([.][0-9]+)*$'
  raw_version="${raw_version#=}"

  if [[ ! "$raw_version" =~ $version_pattern ]]; then
    fail "expected an exact PostgreSQL version, got '${raw_version}'"
  fi

  printf '%s\n' "$raw_version"
}

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

release_base_url() {
  local raw_url="${POSTGRESQL_RELEASES_URL:-https://github.com/theseus-rs/postgresql-binaries}"
  raw_url="${raw_url%/}"
  raw_url="${raw_url%/releases}"
  printf '%s\n' "$raw_url"
}

cache_is_complete() {
  local version_dir="$1"
  [[ -f "${version_dir}/.complete" && -x "${version_dir}/bin/postgres" ]]
}

capture_existing_exit_handler() {
  local trap_spec

  trap_spec="$(trap -p EXIT || true)"

  if [[ -z "$trap_spec" ]]; then
    return
  fi

  trap_spec="${trap_spec#trap -- }"
  trap_spec="${trap_spec% EXIT}"

  # Bash reports trap bodies as shell-quoted strings. Evaluate only that
  # already-installed handler so the new cleanup can compose with it. This
  # assumes earlier EXIT handlers came from trusted code; an untrusted
  # preinstalled EXIT trap would make this evaluation unsafe.
  eval "PREVIOUS_EXIT_HANDLER=${trap_spec}"
}

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

register_exit_cleanup() {
  if [[ "$EXIT_CLEANUP_REGISTERED" == true ]]; then
    return
  fi

  capture_existing_exit_handler
  trap run_exit_cleanup EXIT
  EXIT_CLEANUP_REGISTERED=true
}

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
  curl --fail --location --retry 5 --retry-all-errors \
    --connect-timeout 30 --max-time 600 --speed-time 60 --speed-limit 1024 \
    --output "${work_dir}/${asset}" \
    "${base_url}/releases/download/${version}/${asset}"
  curl --fail --location --retry 5 --retry-all-errors \
    --connect-timeout 30 --max-time 600 --speed-time 60 --speed-limit 1024 \
    --output "${work_dir}/${asset}.sha256" \
    "${base_url}/releases/download/${version}/${asset}.sha256"

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

main() {
  local version
  local root_dir
  local version_dir

  version="$(normalise_version)"
  root_dir="$(cache_dir)"
  version_dir="${root_dir}/${version}"
  mkdir -p "$root_dir"
  acquire_cache_lock "$root_dir"

  if cache_is_complete "$version_dir"; then
    log "cache hit for PostgreSQL ${version} at ${version_dir}"
    return
  fi

  if populate_from_theseus_cache "$version" "$version_dir"; then
    log "cache warmed for PostgreSQL ${version} at ${version_dir}"
    return
  fi

  download_and_extract "$version" "$version_dir" "$(platform_triple)" "$(release_base_url)"
  log "cache warmed for PostgreSQL ${version} at ${version_dir}"
}

main "$@"
