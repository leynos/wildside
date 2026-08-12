#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["pytest"]
# ///
"""Tests for the warm-up script's download, checksum, and install paths.

These cover fetching a release archive, verifying its checksum, installing the
extracted cache, resolving the release URL, and the end-to-end ``main`` flow.
"""

from __future__ import annotations

import hashlib
import os
import shlex
import tarfile
import typing as typ

import pytest

from scripts.warm_pg_embedded_cache_support import result_diagnostics, run_bash

if typ.TYPE_CHECKING:
    import subprocess
    from pathlib import Path


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
