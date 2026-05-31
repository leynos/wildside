#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["pytest", "hypothesis"]
# ///
"""Tests for the embedded PostgreSQL cache warm-up shell script."""

from __future__ import annotations

import hashlib
import os
import subprocess
import tarfile
from pathlib import Path

from hypothesis import given, settings
from hypothesis import strategies as st
import pytest


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
    return subprocess.run(
        ["bash", "-c", f"source {SCRIPT_PATH} && {snippet}"],
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
def test_normalise_version_accepts_exact_versions(
    env: dict[str, str], expected: str
) -> None:
    result = run_bash("normalise_version", env=env)

    assert result.returncode == 0, result_diagnostics(result)
    assert result.stdout.strip() == expected, result_diagnostics(result)


def test_normalise_version_rejects_non_numeric_values() -> None:
    result = run_bash(
        "normalise_version",
        env={"PG_EMBEDDED_VERSION": "", "POSTGRESQL_VERSION": "main"},
    )

    assert result.returncode != 0, (
        f"normalise_version should reject non-numeric value; "
        f"got {result_diagnostics(result)}"
    )
    assert "expected an exact PostgreSQL version" in result.stderr, (
        result_diagnostics(result)
    )


def test_normalise_version_prefers_pg_embedded_version() -> None:
    result = run_bash(
        "normalise_version",
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
def test_normalise_version_accepts_all_valid_numeric_versions(
    major: int, minor: int, patch: int
) -> None:
    """normalise_version accepts any dot-separated numeric triple."""

    version = f"{major}.{minor}.{patch}"
    result = run_bash(
        "normalise_version",
        env={"POSTGRESQL_VERSION": version},
    )

    assert result.returncode == 0, (
        f"normalise_version rejected valid version '{version}'; "
        f"{result_diagnostics(result)}"
    )
    assert result.stdout.strip() == version, (
        f"normalise_version returned '{result.stdout.strip()}' for input '{version}'; "
        f"{result_diagnostics(result)}"
    )


@given(
    version=st.one_of(
        st.just("main"),
        st.just("latest"),
        st.just("16.a.0"),
        st.just("alpha"),
        st.from_regex(r"[^0-9.][^\s]*", fullmatch=True).filter(
            lambda s: "\x00" not in s and s.strip() != ""
        ),
    )
)
@settings(max_examples=30)
def test_normalise_version_rejects_all_non_numeric_versions(
    version: str,
) -> None:
    """normalise_version rejects every non-numeric or non-exact version string."""

    result = run_bash(
        "normalise_version",
        env={"POSTGRESQL_VERSION": version},
    )

    assert result.returncode != 0, (
        f"normalise_version should reject non-numeric version '{version}'; "
        f"got {result_diagnostics(result)}"
    )
    assert "expected an exact PostgreSQL version" in result.stderr, (
        f"normalise_version should reject '{version}' with the expected message; "
        f"{result_diagnostics(result)}"
    )


def test_acquire_cache_lock_removes_stale_lock(tmp_path: Path) -> None:
    lock_dir = tmp_path / ".warm-pg-embedded-cache.lock"
    lock_dir.mkdir()
    (lock_dir / "pid").write_text("99999999\n", encoding="utf-8")

    result = run_bash(
        f"acquire_cache_lock {tmp_path}; [[ -d \"$CACHE_LOCK_DIR\" ]] && echo acquired"
    )

    assert result.returncode == 0, result_diagnostics(result)
    assert result.stdout.strip() == "acquired", result_diagnostics(result)
    assert "removing stale PostgreSQL cache lock" in result.stderr, (
        result_diagnostics(result)
    )


def test_acquire_cache_lock_waits_for_live_lock(tmp_path: Path) -> None:
    lock_dir = tmp_path / ".warm-pg-embedded-cache.lock"
    lock_dir.mkdir()
    (lock_dir / "pid").write_text(f"{os.getpid()}\n", encoding="utf-8")

    result = run_bash(
        f"sleep() {{ exit 77; }}; acquire_cache_lock {tmp_path}",
    )

    assert result.returncode == 77, result_diagnostics(result)
    assert "waiting for cache lock" in result.stderr, result_diagnostics(result)


def test_acquire_cache_lock_treats_missing_pid_as_contended(tmp_path: Path) -> None:
    (tmp_path / ".warm-pg-embedded-cache.lock").mkdir()

    result = run_bash(
        f"sleep() {{ exit 77; }}; acquire_cache_lock {tmp_path}",
    )

    assert result.returncode == 77, result_diagnostics(result)
    assert "waiting for cache lock" in result.stderr, result_diagnostics(result)


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
    asset = tmp_path / "postgresql-16.10.0-x86_64-unknown-linux-gnu.tar.gz"
    write_archive(asset, include_postgres=True)
    write_checksum(asset)

    result = run_bash(
        f"verify_checksum {tmp_path} {asset.name} x86_64-unknown-linux-gnu"
    )

    assert result.returncode == 0, result_diagnostics(result)


def test_verify_checksum_rejects_mismatched_sha256(tmp_path: Path) -> None:
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
    result = run_download_with_fixture(
        tmp_path, curl_stub, include_postgres=False
    )

    assert result.returncode != 0, result_diagnostics(result)
    assert "archive did not contain bin/postgres" in result.stderr, (
        result_diagnostics(result)
    )


def test_download_and_extract_installs_complete_cache(
    tmp_path: Path, curl_stub: Path
) -> None:
    result = run_download_with_fixture(tmp_path, curl_stub, include_postgres=True)

    version_dir = tmp_path / "cache" / "16.10.0"
    assert result.returncode == 0, result_diagnostics(result)
    assert (version_dir / ".complete").is_file()
    assert os.access(version_dir / "bin" / "postgres", os.X_OK)


def test_download_and_extract_reports_curl_failures(
    tmp_path: Path, curl_stub: Path
) -> None:
    failed_asset = "postgresql-16.10.0-x86_64-unknown-linux-gnu.tar.gz"
    result = run_download_with_fixture(
        tmp_path,
        curl_stub,
        include_postgres=True,
        fail_asset=failed_asset,
    )

    assert result.returncode != 0, result_diagnostics(result)
    assert failed_asset in result.stderr, result_diagnostics(result)
    assert "https://example.invalid/theseus/releases/download/16.10.0" in result.stderr, (
        result_diagnostics(result)
    )
    assert "curl exit 23" in result.stderr, result_diagnostics(result)
    assert f"cache root: {tmp_path / 'cache'}" in result.stderr, (
        result_diagnostics(result)
    )


def test_install_cache_dir_replaces_existing_directory(tmp_path: Path) -> None:
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
    assert list(tmp_path.glob("16.10.0.previous.*")) == []


def test_install_cache_dir_restores_previous_directory_when_final_mv_fails(
    tmp_path: Path,
) -> None:
    prepared_dir = tmp_path / "prepared"
    version_dir = tmp_path / "16.10.0"
    prepared_dir.mkdir()
    version_dir.mkdir()
    (prepared_dir / "new").write_text("new\n", encoding="utf-8")
    (version_dir / "old").write_text("old\n", encoding="utf-8")

    result = run_bash(
        "MV_COUNT=0; "
        "mv() { MV_COUNT=$((MV_COUNT + 1)); "
        "if ((MV_COUNT == 2)); then return 1; fi; command mv \"$@\"; }; "
        f"install_cache_dir {prepared_dir} {version_dir}"
    )

    assert result.returncode != 0, result_diagnostics(result)
    assert (version_dir / "old").is_file()
    assert not (version_dir / "new").exists()
    assert not prepared_dir.exists()


# NOTE: An end-to-end test that exercises the actual GitHub Releases download
# boundary is infeasible in the unit test suite: it would require network access
# to https://github.com/theseus-rs/postgresql-binaries/releases, introduce
# non-deterministic latency, and duplicate the CI warm-up step itself.
# The test below validates main() by stubbing curl with a local fixture.


def test_main_warms_cache_from_local_fixtures(
    tmp_path: Path, curl_stub: Path
) -> None:
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
