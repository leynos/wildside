#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["pytest", "hypothesis"]
# ///
"""Tests for the embedded PostgreSQL cache warm-up shell script."""

# NOTE: backend/tests/support/atexit_cleanup.rs::shared_cluster_handle() is not
# unit-tested here because it requires a live embedded PostgreSQL cluster.
# Coverage is provided end-to-end by every integration-test binary in the
# pg-embed nextest group (e.g., catalogue_descriptor_ingestion_bdd). A unit test
# that mocks the cluster handle would not exercise any meaningful behaviour.

from __future__ import annotations

import hashlib
import os
import shlex
import subprocess  # noqa: S404 -- test harness invokes a fixed, trusted script
import tarfile
from pathlib import Path

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = PROJECT_ROOT / "scripts" / "warm-pg-embedded-cache.sh"


def result_diagnostics(result: subprocess.CompletedProcess[str]) -> str:
    """Format subprocess output for actionable assertion failures."""
    return (
        f"returncode={result.returncode}; "
        f"stdout={result.stdout!r}; stderr={result.stderr!r}"
    )


def run_bash(
    snippet: str,
    *,
    env: dict[str, str] | None = None,
    timeout: float = 5.0,
) -> subprocess.CompletedProcess[str]:
    """Source the warm-up script and run a Bash snippet."""
    merged_env = os.environ.copy()
    merged_env.pop("PG_EMBEDDED_VERSION", None)
    merged_env.pop("POSTGRESQL_VERSION", None)
    merged_env.pop("PG_BINARY_CACHE_DIR", None)
    merged_env.pop("POSTGRESQL_RELEASES_URL", None)
    if env is not None:
        merged_env.update(env)
    return subprocess.run(  # noqa: S603 -- args are test-controlled, not external input
        ["bash", "-c", f"source {SCRIPT_PATH} && {snippet}"],  # noqa: S607
        cwd=PROJECT_ROOT,
        env=merged_env,
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
    )


@pytest.mark.parametrize(
    ("env", "expected"),
    [
        ({"PG_EMBEDDED_VERSION": "", "POSTGRESQL_VERSION": "16.10.0"}, "16.10.0"),
        ({"PG_EMBEDDED_VERSION": "", "POSTGRESQL_VERSION": "=16.10.0"}, "16.10.0"),
        ({}, "16.10.0"),
    ],
)
def test_normalize_version_accepts_exact_versions(
    env: dict[str, str], expected: str
) -> None:
    """normalize_version passes through already-exact PostgreSQL versions."""
    result = run_bash("normalize_version", env=env)

    assert result.returncode == 0, result_diagnostics(result)
    assert result.stdout.strip() == expected, result_diagnostics(result)


def test_normalize_version_rejects_non_numeric_values() -> None:
    """normalize_version rejects a non-numeric PostgreSQL version string."""
    result = run_bash(
        "normalize_version",
        env={"PG_EMBEDDED_VERSION": "", "POSTGRESQL_VERSION": "main"},
    )

    assert result.returncode != 0, (
        f"normalize_version should reject non-numeric value; "
        f"got {result_diagnostics(result)}"
    )
    assert "expected an exact PostgreSQL version" in result.stderr, result_diagnostics(
        result
    )


def test_normalize_version_prefers_pg_embedded_version() -> None:
    """PG_EMBEDDED_VERSION takes precedence over POSTGRESQL_VERSION."""
    result = run_bash(
        "normalize_version",
        env={"PG_EMBEDDED_VERSION": "=16.11.0", "POSTGRESQL_VERSION": "16.10.0"},
    )

    assert result.returncode == 0, result_diagnostics(result)
    assert result.stdout.strip() == "16.11.0", result_diagnostics(result)


@given(
    major=st.integers(min_value=1, max_value=99),
    minor=st.integers(min_value=0, max_value=99),
    patch=st.integers(min_value=0, max_value=99),
)
@settings(max_examples=50)
def test_normalize_version_accepts_all_valid_numeric_versions(
    major: int, minor: int, patch: int
) -> None:
    """normalize_version accepts any dot-separated numeric triple."""
    version = f"{major}.{minor}.{patch}"
    result = run_bash(
        "normalize_version",
        env={"POSTGRESQL_VERSION": version},
    )

    assert result.returncode == 0, (
        f"normalize_version rejected valid version '{version}'; "
        f"{result_diagnostics(result)}"
    )
    assert result.stdout.strip() == version, (
        f"normalize_version returned '{result.stdout.strip()}' for input '{version}'; "
        f"{result_diagnostics(result)}"
    )


@given(
    version=st.one_of(
        st.just("main"),
        st.just("latest"),
        st.just("16.a.0"),
        st.just("alpha"),
        st.from_regex(r"\d+\.\d+$", fullmatch=True),
        st.from_regex(r"\d+\.\d+\.\d+\.\d+$", fullmatch=True),
        st.from_regex(r"\d+\.\d+\.\d+[A-Za-z-][^\s]*", fullmatch=True).filter(
            lambda s: "\x00" not in s
        ),
        st.from_regex(r"=\d+\.\d+$", fullmatch=True).filter(
            lambda s: "\x00" not in s and s.strip()
        ),
        st.from_regex(r"=\d+\.\d+\.\d+\.\d+$", fullmatch=True).filter(
            lambda s: "\x00" not in s and s.strip()
        ),
        st.from_regex(r"=\d+\.\d+\.\d+[A-Za-z-][^\s]*", fullmatch=True).filter(
            lambda s: "\x00" not in s and s.strip()
        ),
        st.from_regex(r"=[^0-9][^\s]*", fullmatch=True).filter(
            lambda s: "\x00" not in s and s.strip()
        ),
        st.from_regex(r"[^0-9.=][^\s]*", fullmatch=True).filter(
            lambda s: "\x00" not in s and s.strip()
        ),
    )
)
@settings(max_examples=30)
def test_normalize_version_rejects_all_non_numeric_versions(
    version: str,
) -> None:
    """normalize_version rejects every non-numeric or non-exact version string."""
    result = run_bash(
        "normalize_version",
        env={"POSTGRESQL_VERSION": version},
    )

    assert result.returncode != 0, (
        f"normalize_version should reject non-numeric version '{version}'; "
        f"got {result_diagnostics(result)}"
    )
    assert "expected an exact PostgreSQL version" in result.stderr, (
        f"normalize_version should reject '{version}' with the expected message; "
        f"{result_diagnostics(result)}"
    )


def test_acquire_cache_lock_removes_stale_lock(tmp_path: Path) -> None:
    """A lock held by a dead PID is removed and the lock is re-acquired."""
    lock_dir = tmp_path / ".warm-pg-embedded-cache.lock"
    lock_dir.mkdir()
    (lock_dir / "pid").write_text("99999999\n", encoding="utf-8")

    result = run_bash(
        f'acquire_cache_lock {tmp_path}; [[ -d "$CACHE_LOCK_DIR" ]] && echo acquired'
    )

    assert result.returncode == 0, result_diagnostics(result)
    assert result.stdout.strip() == "acquired", result_diagnostics(result)
    assert "removing stale PostgreSQL cache lock" in result.stderr, result_diagnostics(
        result
    )


def test_acquire_cache_lock_waits_for_live_lock(tmp_path: Path) -> None:
    """A lock held by a live PID causes the caller to wait rather than steal it."""
    lock_dir = tmp_path / ".warm-pg-embedded-cache.lock"
    lock_dir.mkdir()
    (lock_dir / "pid").write_text(f"{os.getpid()}\n", encoding="utf-8")

    result = run_bash(
        f"sleep() {{ exit 77; }}; acquire_cache_lock {tmp_path}",
    )

    assert result.returncode == 77, result_diagnostics(result)
    assert "waiting for cache lock" in result.stderr, result_diagnostics(result)


def test_acquire_cache_lock_treats_missing_pid_as_contended(tmp_path: Path) -> None:
    """A lock directory without a pid file is treated as contended, not stale."""
    (tmp_path / ".warm-pg-embedded-cache.lock").mkdir()

    result = run_bash(
        f"sleep() {{ exit 77; }}; acquire_cache_lock {tmp_path}",
    )

    assert result.returncode == 77, result_diagnostics(result)
    assert "waiting for cache lock" in result.stderr, result_diagnostics(result)


def test_remove_stale_cache_lock_reports_contention_when_dir_remains(
    tmp_path: Path,
) -> None:
    """Removal fails and the lock directory is preserved when it is not empty."""
    lock_dir = tmp_path / ".warm-pg-embedded-cache.lock"
    lock_dir.mkdir()
    (lock_dir / "pid").write_text("99999999\n", encoding="utf-8")
    (lock_dir / "unexpected").write_text(
        "keeps rmdir from succeeding", encoding="utf-8"
    )

    result = run_bash(
        f"remove_stale_cache_lock {lock_dir}",
    )

    assert result.returncode != 0, result_diagnostics(result)
    assert lock_dir.exists(), "contended lock directory should remain for retry"


def test_acquire_cache_lock_handles_concurrent_stale_removal(tmp_path: Path) -> None:
    """Two concurrent processes racing to remove a stale lock both succeed safely."""
    import threading

    lock_dir = tmp_path / ".warm-pg-embedded-cache.lock"
    # Seed a stale lock with a guaranteed-dead PID (PID 1 is init and is never
    # dead, so use a PID well outside the kernel range that will never exist).
    lock_dir.mkdir()
    (lock_dir / "pid").write_text("999999999\n")

    results: list[int] = []
    lock = threading.Lock()

    def run_removal() -> None:
        """Attempt stale-lock removal once and record the exit code."""
        result = run_bash(
            f"remove_stale_cache_lock {lock_dir}",
            timeout=5.0,
        )
        with lock:
            results.append(result.returncode)

    threads = [threading.Thread(target=run_removal) for _ in range(4)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()

    # At least one thread must have succeeded; zero failures are also acceptable
    # (if the first thread already removed the lock, subsequent ones see ENOENT
    # and return non-zero, which is the correct safe outcome).
    assert any(result == 0 for result in results), (
        f"at least one concurrent removal should succeed; results: {results}"
    )
    assert not lock_dir.exists(), (
        f"lock directory should not exist after all removals; {lock_dir}"
    )


def write_archive(path: Path, *, include_postgres: bool) -> None:
    """Create a small PostgreSQL-style tar.gz fixture."""
    source_dir = path.parent / f"{path.stem}-source"
    bin_dir = source_dir / "postgresql" / "bin"
    bin_dir.mkdir(parents=True)
    if include_postgres:
        postgres = bin_dir / "postgres"
        postgres.write_text("#!/usr/bin/env sh\nexit 0\n", encoding="utf-8")
        postgres.chmod(0o755)
    with tarfile.open(path, "w:gz") as archive:
        archive.add(source_dir / "postgresql", arcname="postgresql")


def write_checksum(path: Path) -> None:
    """Write a SHA-256 sidecar compatible with sha256sum and shasum."""
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    path.with_name(f"{path.name}.sha256").write_text(
        f"{digest}  {path.name}\n", encoding="utf-8"
    )


def test_verify_checksum_accepts_matching_sha256(tmp_path: Path) -> None:
    """A checksum sidecar matching the archive digest is accepted."""
    asset = tmp_path / "postgresql-16.10.0-x86_64-unknown-linux-gnu.tar.gz"
    write_archive(asset, include_postgres=True)
    write_checksum(asset)

    result = run_bash(
        f"verify_checksum {tmp_path} {asset.name} x86_64-unknown-linux-gnu"
    )

    assert result.returncode == 0, result_diagnostics(result)


def test_verify_checksum_rejects_mismatched_sha256(tmp_path: Path) -> None:
    """A checksum sidecar that does not match the archive is rejected."""
    asset = tmp_path / "postgresql-16.10.0-x86_64-unknown-linux-gnu.tar.gz"
    write_archive(asset, include_postgres=True)
    asset.with_name(f"{asset.name}.sha256").write_text(
        f"{'0' * 64}  {asset.name}\n", encoding="utf-8"
    )

    result = run_bash(
        f"verify_checksum {tmp_path} {asset.name} x86_64-unknown-linux-gnu"
    )

    assert result.returncode != 0, result_diagnostics(result)
    assert "checksum verification failed" in result.stderr, result_diagnostics(result)


def test_verify_checksum_rejects_missing_sha256(tmp_path: Path) -> None:
    """Verification fails when no checksum sidecar file is present."""
    asset = tmp_path / "postgresql-16.10.0-x86_64-unknown-linux-gnu.tar.gz"
    write_archive(asset, include_postgres=True)

    result = run_bash(
        f"verify_checksum {tmp_path} {asset.name} x86_64-unknown-linux-gnu"
    )

    assert result.returncode != 0, result_diagnostics(result)


@pytest.fixture
def curl_stub(tmp_path: Path) -> Path:
    """Place a curl stub ahead of the real executable on PATH."""
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    stub = bin_dir / "curl"
    stub.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
output=''
while (($# > 0)); do
  case "$1" in
    --output)
      output="$2"
      shift 2
      ;;
    *)
      url="$1"
      shift
      ;;
  esac
done
asset="${url##*/}"
if [[ "${CURL_FAIL_ASSET:-}" == "$asset" ]]; then
  exit 23
fi
cp "${CURL_FIXTURE_DIR}/${asset}" "$output"
""",
        encoding="utf-8",
    )
    stub.chmod(0o755)
    return bin_dir


def run_download_with_fixture(
    tmp_path: Path,
    curl_stub: Path,
    *,
    include_postgres: bool,
    fail_asset: str | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run `download_and_extract` with curl redirected to local fixtures."""
    fixture_dir = tmp_path / "fixtures"
    fixture_dir.mkdir()
    version = "16.10.0"
    triple = "x86_64-unknown-linux-gnu"
    asset = fixture_dir / f"postgresql-{version}-{triple}.tar.gz"
    write_archive(asset, include_postgres=include_postgres)
    write_checksum(asset)
    version_dir = tmp_path / "cache" / version
    version_dir.parent.mkdir()
    env = {
        "CURL_FIXTURE_DIR": str(fixture_dir),
        "PATH": f"{curl_stub}:{os.environ['PATH']}",
    }
    if fail_asset is not None:
        env["CURL_FAIL_ASSET"] = fail_asset
    return run_bash(
        "download_and_extract "
        f"{version} {version_dir} {triple} https://example.invalid/theseus",
        env=env,
    )


def test_download_and_extract_rejects_archive_without_postgres(
    tmp_path: Path, curl_stub: Path
) -> None:
    """An archive lacking bin/postgres is rejected after extraction."""
    result = run_download_with_fixture(tmp_path, curl_stub, include_postgres=False)

    assert result.returncode != 0, result_diagnostics(result)
    assert "archive did not contain bin/postgres" in result.stderr, result_diagnostics(
        result
    )


def test_download_and_extract_installs_complete_cache(
    tmp_path: Path, curl_stub: Path
) -> None:
    """A valid archive is downloaded, verified, and installed into the cache."""
    result = run_download_with_fixture(tmp_path, curl_stub, include_postgres=True)

    version_dir = tmp_path / "cache" / "16.10.0"
    assert result.returncode == 0, result_diagnostics(result)
    assert (version_dir / ".complete").is_file()
    assert os.access(version_dir / "bin" / "postgres", os.X_OK)


def test_download_and_extract_reports_curl_failures(
    tmp_path: Path, curl_stub: Path
) -> None:
    """A curl failure is reported with the asset, URL, and exit code."""
    failed_asset = "postgresql-16.10.0-x86_64-unknown-linux-gnu.tar.gz"
    result = run_download_with_fixture(
        tmp_path,
        curl_stub,
        include_postgres=True,
        fail_asset=failed_asset,
    )

    assert result.returncode != 0, result_diagnostics(result)
    assert failed_asset in result.stderr, result_diagnostics(result)
    assert (
        "https://example.invalid/theseus/releases/download/16.10.0" in result.stderr
    ), result_diagnostics(result)
    assert "curl exit 23" in result.stderr, result_diagnostics(result)
    assert f"cache root: {tmp_path / 'cache'}" in result.stderr, result_diagnostics(
        result
    )


def test_install_cache_dir_replaces_existing_directory(tmp_path: Path) -> None:
    """The prepared directory atomically replaces the existing version directory."""
    prepared_dir = tmp_path / "prepared"
    version_dir = tmp_path / "16.10.0"
    prepared_dir.mkdir()
    version_dir.mkdir()
    (prepared_dir / "new").write_text("new\n", encoding="utf-8")
    (version_dir / "old").write_text("old\n", encoding="utf-8")

    result = run_bash(f"install_cache_dir {prepared_dir} {version_dir}")

    assert result.returncode == 0, result_diagnostics(result)
    assert (version_dir / "new").is_file()
    assert not (version_dir / "old").exists()
    assert not list(tmp_path.glob("16.10.0.previous.*"))


def test_install_cache_dir_restores_previous_directory_when_final_mv_fails(
    tmp_path: Path,
) -> None:
    """The original directory is restored if the final move step fails."""
    prepared_dir = tmp_path / "prepared"
    version_dir = tmp_path / "16.10.0"
    prepared_dir.mkdir()
    version_dir.mkdir()
    (prepared_dir / "new").write_text("new\n", encoding="utf-8")
    (version_dir / "old").write_text("old\n", encoding="utf-8")

    result = run_bash(
        "MV_COUNT=0; "
        "mv() { MV_COUNT=$((MV_COUNT + 1)); "
        'if ((MV_COUNT == 2)); then return 1; fi; command mv "$@"; }; '
        f"install_cache_dir {prepared_dir} {version_dir}"
    )

    assert result.returncode != 0, result_diagnostics(result)
    assert (version_dir / "old").is_file()
    assert not (version_dir / "new").exists()
    assert not prepared_dir.exists()


def test_cache_is_complete_returns_false_for_missing_marker(tmp_path: Path) -> None:
    """A cache directory without a .complete marker is not considered complete."""
    version_dir = tmp_path / "16.10.0"
    version_dir.mkdir()
    (version_dir / "bin").mkdir()
    postgres = version_dir / "bin" / "postgres"
    postgres.write_bytes(b"\x7fELF")
    postgres.chmod(0o755)
    result = run_bash(
        f"cache_is_complete {version_dir}",
    )
    assert result.returncode != 0, (
        f"cache_is_complete should return non-zero without .complete; "
        f"{result_diagnostics(result)}"
    )


def test_cache_is_complete_returns_false_for_non_executable_postgres(
    tmp_path: Path,
) -> None:
    """A cache with a non-executable postgres binary is not considered complete."""
    version_dir = tmp_path / "16.10.0"
    version_dir.mkdir()
    (version_dir / "bin").mkdir()
    postgres = version_dir / "bin" / "postgres"
    postgres.write_bytes(b"\x7fELF")
    postgres.chmod(0o644)
    (version_dir / ".complete").write_text("")
    result = run_bash(
        f"cache_is_complete {version_dir}",
    )
    assert result.returncode != 0, (
        f"cache_is_complete should return non-zero without executable postgres; "
        f"{result_diagnostics(result)}"
    )


def test_cache_is_complete_returns_true_for_complete_cache(tmp_path: Path) -> None:
    """A cache with the marker file and an executable postgres is complete."""
    version_dir = tmp_path / "16.10.0"
    version_dir.mkdir()
    (version_dir / "bin").mkdir()
    postgres = version_dir / "bin" / "postgres"
    postgres.write_bytes(b"\x7fELF")
    postgres.chmod(0o755)
    (version_dir / ".complete").write_text("")
    result = run_bash(
        f"cache_is_complete {version_dir}",
    )
    assert result.returncode == 0, (
        f"cache_is_complete should succeed for complete cache; "
        f"{result_diagnostics(result)}"
    )


def test_platform_triple_returns_non_empty_string() -> None:
    """platform_triple emits a non-empty hyphenated target triple."""
    result = run_bash("platform_triple")
    assert result.returncode == 0, (
        f"platform_triple failed; {result_diagnostics(result)}"
    )
    triple = result.stdout.strip()
    assert triple, "platform_triple should return a non-empty string"
    assert "-" in triple, (
        f"platform_triple should return a triple with hyphens, got '{triple}'"
    )


def test_release_base_url_defaults_to_theseus() -> None:
    """Without an override, the release URL points at the theseus-rs mirror."""
    result = run_bash(
        "release_base_url",
        env={"POSTGRESQL_RELEASES_URL": ""},
    )
    assert result.returncode == 0, result_diagnostics(result)
    url = result.stdout.strip()
    assert "theseus-rs/postgresql-binaries" in url, (
        f"default release URL should point to theseus-rs; got '{url}'"
    )


def test_release_base_url_respects_override() -> None:
    """POSTGRESQL_RELEASES_URL overrides the default release base URL."""
    custom = "https://example.invalid/custom-mirror"
    result = run_bash(
        "release_base_url",
        env={"POSTGRESQL_RELEASES_URL": custom},
    )
    assert result.returncode == 0, result_diagnostics(result)
    assert result.stdout.strip() == custom, (
        f"release_base_url should return the override; got '{result.stdout.strip()}'"
    )


def test_populate_from_theseus_cache_copies_when_source_complete(
    tmp_path: Path,
) -> None:
    """A complete theseus cache entry is copied into the destination cache."""
    version = "16.10.0"
    theseus_dir = tmp_path / ".theseus" / "postgresql" / version
    (theseus_dir / "bin").mkdir(parents=True)
    postgres = theseus_dir / "bin" / "postgres"
    postgres.write_bytes(b"\x7fELF")
    postgres.chmod(0o755)
    (theseus_dir / ".complete").write_text("")

    dest_dir = tmp_path / "cache" / version
    dest_dir.parent.mkdir()
    quoted_dest_dir = shlex.quote(str(dest_dir))
    result = run_bash(
        f"populate_from_theseus_cache {version} {quoted_dest_dir}",
        env={"HOME": str(tmp_path)},
    )
    assert result.returncode == 0, result_diagnostics(result)
    assert (dest_dir / ".complete").is_file(), "dest should have .complete marker"
    assert os.access(dest_dir / "bin" / "postgres", os.X_OK), (
        "dest should have executable bin/postgres"
    )


def test_populate_from_theseus_cache_skips_when_source_missing(
    tmp_path: Path,
) -> None:
    """No destination cache is created when the theseus source is absent."""
    version = "16.10.0"
    dest_dir = tmp_path / "cache" / version
    theseus_root = tmp_path / ".theseus" / "postgresql"
    theseus_root.mkdir(parents=True)
    quoted_dest_dir = shlex.quote(str(dest_dir))
    result = run_bash(
        f"populate_from_theseus_cache {version} {quoted_dest_dir}",
        env={"HOME": str(tmp_path)},
    )
    assert result.returncode != 0, result_diagnostics(result)
    assert not dest_dir.exists(), "dest should not be created when source is missing"


# NOTE: An end-to-end test that exercises the actual GitHub Releases download
# boundary is infeasible in the unit test suite: it would require network access
# to https://github.com/theseus-rs/postgresql-binaries/releases, introduce
# non-deterministic latency, and duplicate the CI warm-up step itself.
# The test below validates main() by stubbing curl with a local fixture.


def test_main_warms_cache_from_local_fixtures(tmp_path: Path, curl_stub: Path) -> None:
    """main() installs a complete cache entry when given a stubbed curl."""
    fixture_dir = tmp_path / "fixtures"
    fixture_dir.mkdir()
    version = "16.10.0"
    triple_result = run_bash("platform_triple")
    assert triple_result.returncode == 0, (
        f"platform_triple failed; {result_diagnostics(triple_result)}"
    )
    triple = triple_result.stdout.strip()
    asset = fixture_dir / f"postgresql-{version}-{triple}.tar.gz"
    write_archive(asset, include_postgres=True)
    write_checksum(asset)
    cache_dir = tmp_path / "cache"
    result = run_bash(
        "main",
        env={
            "POSTGRESQL_VERSION": version,
            "PG_BINARY_CACHE_DIR": str(cache_dir),
            "POSTGRESQL_RELEASES_URL": "https://example.invalid/theseus",
            "CURL_FIXTURE_DIR": str(fixture_dir),
            "PATH": f"{curl_stub}:{os.environ['PATH']}",
        },
        timeout=15.0,
    )

    version_dir = cache_dir / version
    assert result.returncode == 0, result_diagnostics(result)
    assert (version_dir / ".complete").is_file(), (
        f"expected .complete marker in {version_dir}; {result_diagnostics(result)}"
    )
    assert os.access(version_dir / "bin" / "postgres", os.X_OK), (
        f"expected executable bin/postgres in {version_dir}"
    )
