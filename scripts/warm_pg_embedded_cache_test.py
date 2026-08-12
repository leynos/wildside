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

import os
import typing as typ

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

from scripts.warm_pg_embedded_cache_support import result_diagnostics, run_bash

if typ.TYPE_CHECKING:
    from pathlib import Path


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
