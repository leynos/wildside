"""Command doubles and helpers shared by the tooling Make contracts.

The Makefile resolves its tools from ``PATH`` and records what it ran through
the doubles defined here. Keeping the harness beside the contracts, rather than
inside one test module, keeps each contract file within the repository's module
size limit.
"""

from __future__ import annotations

import os
import re
import subprocess  # ruff: ignore[suspicious-subprocess-import] - tests deliberately exercise Make via subprocess.
from pathlib import Path
from shutil import which

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]

FAKE_TOOL = """#!/bin/sh
# The Biome double answers the Makefile's version probe without logging it, so
# a contract can assert the lint invocation alone. `FAKE_BIOME_VERSION` is the
# version the double claims; unset means "not installed at the pinned version".
if [ "$(basename "$0")" = "biome" ] && [ "$1" = "--version" ] \\
    && [ -n "${FAKE_BIOME_VERSION:-}" ]; then
    printf 'Version: %s\\n' "$FAKE_BIOME_VERSION"
    exit 0
fi
printf '%s\\0%s\\0%s\\0%s\\0%s\\0' \\
    "$(basename "$0")" \\
    "${TMPDIR:-}" \\
    "${UV_CACHE_DIR:-}" \\
    "${UV_TOOL_DIR:-}" \\
    "$#" >> "$TOOL_LOG"
for argument in "$@"; do
    printf '%s\\0' "$argument" >> "$TOOL_LOG"
done
"""

type ToolInvocation = tuple[str, str, str, str, tuple[str, ...]]


class ToolingDoubleError(RuntimeError):
    """The tooling doubles cannot run the requested contract."""


def _write_executable(path: Path, source: str) -> None:
    """Write an executable command double."""
    path.write_text(source, encoding="utf8")
    path.chmod(0o755)


#: The temporary directory the fixture hands to every Make invocation. The
#: assertions compare the tools' `TMPDIR` against this, so the contract is
#: "the Makefile passes the caller's TMPDIR through untouched" rather than
#: "whoever ran the suite happened to have no TMPDIR set". Reading the
#: ambient value made the suite pass on GitHub's runners, which set none, and
#: fail on any developer machine that sets one.
TMPDIR_SENTINEL: str = "tmpdir-owned-by-the-fixture"


def makefile_variable(name: str) -> str:
    """Return the value of a ``?=`` variable defined in the Makefile.

    The Biome double has to claim the version the Makefile itself pins, or the
    contract would encode the pin twice and drift from it.

    Parameters
    ----------
    name : str
        Makefile variable name.

    Returns
    -------
    str
        Value assigned by the matching ``?=`` definition.

    Raises
    ------
    ToolingDoubleError
        If the Makefile does not define ``name`` with ``?=``.
    """
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf8")
    match = re.search(rf"^{re.escape(name)} \?= (\S+)$", makefile, re.MULTILINE)
    if match is None:
        message = f"the Makefile must define {name} with ?="
        raise ToolingDoubleError(message)
    return match.group(1)


def build_fake_tool_environment(tmp_path: Path) -> tuple[dict[str, str], Path]:
    """Build the fake-tool environment and invocation-log path.

    Parameters
    ----------
    tmp_path : Path
        Temporary directory owned by the test fixture.

    Returns
    -------
    tuple[dict[str, str], Path]
        Environment mapping and invocation-log path.
    """
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _write_executable(fake_bin / "tool", FAKE_TOOL)
    for tool_name in ("bun", "pnpm", "uv", "nixie", "merman-cli", "biome"):
        (fake_bin / tool_name).symlink_to(fake_bin / "tool")

    temporary = tmp_path / TMPDIR_SENTINEL
    temporary.mkdir()

    log_path = tmp_path / "tool.log"
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
    env["TOOL_LOG"] = str(log_path)
    env["TMPDIR"] = str(temporary)
    # Make's `?=` keeps a caller-exported BIOME_VERSION, so pin it here as
    # well: the fake Biome and the Makefile must agree on the version even
    # when the surrounding environment exports a different one.
    biome_version = makefile_variable("BIOME_VERSION")
    env["BIOME_VERSION"] = biome_version
    env["FAKE_BIOME_VERSION"] = biome_version
    return env, log_path


def run_make(
    target: str,
    env: dict[str, str],
    *,
    cwd: Path = REPOSITORY_ROOT,
) -> subprocess.CompletedProcess[str]:
    """Run a Makefile tooling target with command doubles on ``PATH``.

    Parameters
    ----------
    target : str
        Make target to run.
    env : dict[str, str]
        Environment passed to Make.
    cwd : Path, default=REPOSITORY_ROOT
        Working directory for Make.

    Returns
    -------
    subprocess.CompletedProcess[str]
        Completed Make process.

    Raises
    ------
    ToolingDoubleError
        If Make is unavailable.
    """
    make = which("make")
    if make is None:
        message = "make must be available for workflow contract tests"
        raise ToolingDoubleError(message)
    fake_uv = Path(env["PATH"].split(os.pathsep, maxsplit=1)[0]) / "uv"
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the resolved make executable is trusted.
        [
            make,
            "--no-print-directory",
            "-f",
            str(REPOSITORY_ROOT / "Makefile"),
            f"PATH={env['PATH']}",
            f"UV={fake_uv}",
            target,
        ],
        cwd=cwd,
        env=env,
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )


def read_invocations(log_path: Path) -> list[ToolInvocation]:
    """Read logged command invocations.

    Parameters
    ----------
    log_path : Path
        Invocation-log path.

    Returns
    -------
    list[ToolInvocation]
        Logged invocations, or an empty list when none ran.
    """
    if not log_path.exists():
        return []
    fields = iter(log_path.read_bytes().removesuffix(b"\0").split(b"\0"))
    invocations = []
    while tool := next(fields, None):
        tmpdir = next(fields).decode()
        uv_cache_dir = next(fields).decode()
        uv_tool_dir = next(fields).decode()
        argument_count = int(next(fields))
        arguments = tuple(next(fields).decode() for _ in range(argument_count))
        invocations.append((
            tool.decode(),
            tmpdir,
            uv_cache_dir,
            uv_tool_dir,
            arguments,
        ))
    return invocations


def tool_arguments(log_path: Path, tool: str) -> list[tuple[str, ...]]:
    """Return arguments for each logged invocation of a tool.

    Parameters
    ----------
    log_path : Path
        Invocation-log path.
    tool : str
        Tool name to filter.

    Returns
    -------
    list[tuple[str, ...]]
        Argument tuples for matching invocations.
    """
    return [
        invocation[-1]
        for invocation in read_invocations(log_path)
        if invocation[0] == tool
    ]
