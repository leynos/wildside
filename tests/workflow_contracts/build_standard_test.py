"""Contract tests for the Rust build standard and the dev-build footprint.

The Concordat ``rust-build-defaults`` rule needs the parallel ``rustc`` frontend
in every ``rustflags`` source, ``mold`` in the Linux source only, the gate
recipes that assign ``RUSTFLAGS`` to restate both, and a runner that has the
linker. The ``rust-dev-build-footprint`` rule needs the development profile to
disable incremental compilation and debug information, and the pinned nightly
to take Cargo's native ``embed-metadata = false``. Cranelift is not adopted
(see "Cranelift is not the development backend" in
``docs/developers-guide.md``), so the configuration must stay free of it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import os
import re
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the contract reads what `make -n` prints.
import tomllib
import typing as typ
from pathlib import Path
from shutil import which

import pytest
import workflow_inventory as inv

ROOT = Path(__file__).resolve().parents[2]
THREADS = "-Zthreads=8"
LINKER_FLAG = "-Clink-arg=-fuse-ld=mold"
LINUX_TABLE = 'cfg(target_os = "linux")'
GATE_TARGETS = ("lint-architecture", "test-rust")
STANDARD_FLAGS = f"-D warnings {THREADS} {LINKER_FLAG}"


def _config() -> dict[str, typ.Any]:
    return tomllib.loads((ROOT / ".cargo" / "config.toml").read_text("utf-8"))


def test_every_rustflags_source_carries_the_parallel_frontend() -> None:
    """``[build]`` and the Linux target table both enable ``-Zthreads=8``."""
    config = _config()
    assert THREADS in config["build"]["rustflags"]
    assert THREADS in config["target"][LINUX_TABLE]["rustflags"]


def test_the_linker_is_configured_for_linux_only() -> None:
    """The linker flag is in the Linux table and in no other source."""
    config = _config()
    assert LINKER_FLAG in config["target"][LINUX_TABLE]["rustflags"]
    assert LINKER_FLAG not in config["build"]["rustflags"]
    others = [name for name in config["target"] if name != LINUX_TABLE]
    assert not others, f"unexpected target tables {others}"


def test_cranelift_stays_out_of_the_configuration() -> None:
    """The recorded exception holds while the configuration omits the backend."""
    config = _config()
    assert "codegen-backend" not in config.get("unstable", {})
    assert "codegen-backend" not in config.get("profile", {}).get("dev", {})


def test_the_development_profile_keeps_the_footprint_settings() -> None:
    """The dev profile disables incremental state and debug information."""
    dev = _config()["profile"]["dev"]
    assert dev.get("incremental") is False, "profile.dev must set incremental = false"
    debug = dev.get("debug")
    # Python equality would also accept ``0.0``, which Cargo rejects, so test the
    # type: the exact integer ``0`` or the boolean ``false``.
    is_zero_integer = (
        isinstance(debug, int) and not isinstance(debug, bool) and debug == 0
    )
    assert debug is False or is_zero_integer, "profile.dev must set debug = 0"


def test_the_configuration_disables_metadata_embedding_through_cargo() -> None:
    """The native Cargo key is set, the old key and raw rustc flags are absent."""
    config = _config()
    unstable = config.get("unstable", {})
    assert unstable.get("embed-metadata") is False
    assert "no-embed-metadata" not in unstable
    flags = [*config["build"]["rustflags"], *config["target"][LINUX_TABLE]["rustflags"]]
    assert not any("embed-metadata" in flag for flag in flags)


def _make_dry_run(
    target: str, host_os: str, env: dict[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    """Run ``make -n`` for ``target`` as if on ``host_os``."""
    environment = {k: v for k, v in os.environ.items() if k != "CARGO_BUILD_TARGET"}
    environment.update(env or {})
    make = which("make")
    assert make is not None, "make must be installed to read the recipes"
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - a resolved `make -n` over this repository.
        [make, "-n", "-s", f"BUILD_HOST_OS={host_os}", target],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )


def _recipe_commands(target: str, host_os: str) -> str:
    """Return the commands ``make -n`` prints for ``target`` on ``host_os``."""
    completed = _make_dry_run(target, host_os)
    assert completed.returncode == 0, completed.stderr
    return completed.stdout


@pytest.mark.parametrize("target", GATE_TARGETS)
def test_gate_recipes_restate_the_standard_flags_on_linux(target: str) -> None:
    """A recipe that assigns ``RUSTFLAGS`` restates the frontend and the linker."""
    commands = _recipe_commands(target, "Linux")
    assert "RUSTFLAGS=" in commands
    assert STANDARD_FLAGS in commands


@pytest.mark.parametrize("target", GATE_TARGETS)
def test_gate_recipes_leave_the_linker_off_other_platforms(target: str) -> None:
    """Off Linux the recipe keeps the frontend flag and drops the linker."""
    commands = _recipe_commands(target, "Darwin")
    assert f"-D warnings {THREADS}" in commands
    assert LINKER_FLAG not in commands


def test_gate_recipes_keep_an_inherited_rustflags() -> None:
    """The recipe appends to an inherited value instead of replacing it."""
    commands = _recipe_commands("test-rust", "Linux")
    assert re.search(r'RUSTFLAGS="\$\{RUSTFLAGS:\+\$RUSTFLAGS \}', commands)


@pytest.mark.parametrize("target", GATE_TARGETS)
def test_gate_recipes_refuse_a_cross_target_invocation(target: str) -> None:
    """The linker flag comes from the host, so a target override is refused."""
    completed = _make_dry_run(
        target, "Linux", env={"CARGO_BUILD_TARGET": "x86_64-pc-windows-gnu"}
    )
    assert completed.returncode != 0
    assert "picks `mold` from the host" in completed.stderr


def test_whitaker_keeps_its_own_flag_boundary() -> None:
    """The Whitaker driver does not inherit the development flags."""
    commands = _recipe_commands("lint-whitaker", "Linux")
    whitaker = [line for line in commands.splitlines() if "whitaker" in line]
    assert whitaker, "lint-whitaker must run Whitaker"
    assert all(THREADS not in line and LINKER_FLAG not in line for line in whitaker)


def _build_job() -> dict[str, typ.Any]:
    return inv.load_workflow("ci.yml")["jobs"]["build"]


def _setup_rust_step(job: dict[str, typ.Any]) -> dict[str, typ.Any]:
    steps = [
        step
        for step in job["steps"]
        if "/actions/setup-rust@" in str(step.get("uses", ""))
    ]
    assert len(steps) == 1, "the build job must run setup-rust once"
    return steps[0]


def test_the_build_job_installs_the_linker_and_exports_the_standard_flags() -> None:
    """setup-rust exports the complete flags, so cargo steps keep the standard."""
    inputs = _setup_rust_step(_build_job())["with"]
    assert str(inputs.get("install-mold")) == "true"
    assert inputs.get("rustflags") == STANDARD_FLAGS


def test_the_dependabot_test_step_restates_the_standard_flags() -> None:
    """The step that assigns ``RUSTFLAGS`` itself carries the same flags."""
    steps = [step for step in _build_job()["steps"] if step.get("name") == "Rust tests"]
    assert len(steps) == 1
    assert f'RUSTFLAGS="{STANDARD_FLAGS}" cargo nextest run' in steps[0]["run"]


def test_the_mutation_workflow_installs_the_linker_and_restores_the_flags() -> None:
    """``setup-rust`` exports ``-D warnings`` alone, so the flags are restored."""
    job = inv.load_workflow("mutation-testing.yml")["jobs"]["mutation"]
    assert str(job["with"].get("install-mold")) == "true"
    assert (
        f'RUSTFLAGS={STANDARD_FLAGS}" >> "$GITHUB_ENV"' in job["with"]["setup-commands"]
    )
