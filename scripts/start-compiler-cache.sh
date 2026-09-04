#!/usr/bin/env bash
# Install a pinned sccache and start its server from a `run:` step.
#
# The server binds its storage backend at the moment it starts, so where it
# starts decides where its objects go. The shared `setup-rust` action starts
# one through `mozilla-actions/sccache-action`, and that action's last act is
# to write `ACTIONS_CACHE_SERVICE_V2=on` back to GITHUB_ENV along with
# GitHub's own results URL and token. That clobbers the credential export this
# job made earlier, for that server and for every step after it, and the
# objects go to GitHub rather than the managed runner's proxy. Calling
# `setup-rust` with `use-sccache: false` keeps that step out of the job, and
# starting the server here means it reads the exported values as they were
# written. Every Rust job calls this script instead of inlining it, so the
# digest pins and the backend guard have one place to change.
#
# Required environment:
#   SCCACHE_VERSION      the release to install, without the leading `v`
#   SCCACHE_SHA256_X64   sha256 of the x86_64-unknown-linux-musl archive
#   SCCACHE_SHA256_ARM64 sha256 of the aarch64-unknown-linux-musl archive
#   RUNNER_ARCH          set by the Actions runner; X64 or ARM64
set -euo pipefail

: "${SCCACHE_VERSION:?SCCACHE_VERSION must be set}"
: "${SCCACHE_SHA256_X64:?SCCACHE_SHA256_X64 must be set}"
: "${SCCACHE_SHA256_ARM64:?SCCACHE_SHA256_ARM64 must be set}"

# The binary lives here whether the tool archive restored it or this script
# installs it below. Runner images differ on whether that directory is already
# on PATH, and the later steps resolve `sccache` through PATH too, because it
# is named as RUSTC_WRAPPER. Put it on PATH for this script and, when running
# under Actions, for every step after it.
install_dir="${HOME}/.local/bin"
case ":${PATH}:" in
  *":${install_dir}:"*) ;;
  *) PATH="${install_dir}:${PATH}"; export PATH ;;
esac
if [ -n "${GITHUB_PATH:-}" ]; then
  printf '%s\n' "$install_dir" >>"$GITHUB_PATH"
fi

version() {
  sccache --version 2>/dev/null | awk 'NR == 1 { print $NF }'
}

# The tool archive that carries ~/.local/bin is keyed on SCCACHE_VERSION, so a
# restored binary already matches the pin. This probe is the belt to that
# braces: it also corrects a binary restored under an older key.
if [ "$(version)" != "$SCCACHE_VERSION" ]; then
  case "${RUNNER_ARCH:-}" in
    X64) target=x86_64-unknown-linux-musl; expected_sha="$SCCACHE_SHA256_X64" ;;
    ARM64) target=aarch64-unknown-linux-musl; expected_sha="$SCCACHE_SHA256_ARM64" ;;
    *) echo "unsupported runner architecture: ${RUNNER_ARCH:-}" >&2; exit 1 ;;
  esac
  archive="sccache-v${SCCACHE_VERSION}-${target}.tar.gz"
  staging="$(mktemp -d)"
  trap 'rm -rf -- "$staging"' EXIT
  curl -fsSL --proto '=https' --tlsv1.2 --retry 3 --retry-all-errors \
    "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/${archive}" \
    -o "${staging}/${archive}"
  actual_sha="$(sha256sum "${staging}/${archive}" | awk '{print $1}')"
  if [ "$actual_sha" != "$expected_sha" ]; then
    echo "sccache digest mismatch: expected ${expected_sha}, got ${actual_sha}" >&2
    exit 1
  fi
  tar -xzf "${staging}/${archive}" -C "$staging" --strip-components=1 \
    "sccache-v${SCCACHE_VERSION}-${target}/sccache"
  mkdir -p "$install_dir"
  install -m 0755 "${staging}/sccache" "${install_dir}/sccache"
fi

test "$(version)" = "$SCCACHE_VERSION"
sccache --start-server

# A server on any backend other than ghac writes somewhere the rest of the
# estate cannot read, so fail rather than measure it.
location="$(sccache --show-stats | awk -F'  +' '/^Cache location/ { print $2 }')"
echo "sccache backend: ${location}"
case "$location" in
  ghac*) ;;
  *) echo "sccache did not bind the Actions cache backend" >&2; exit 1 ;;
esac
