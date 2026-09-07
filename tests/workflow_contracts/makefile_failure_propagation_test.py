"""Contract tests proving a failing recipe line fails its Make target.

`.ONESHELL` is a global special target: GNU make ignores its prerequisite
list, so naming a single target enables one-shell recipes for the whole file.
Every multi-line recipe then reaches the shell as one script, and under make's
default `.SHELLFLAGS` of `-c` that script's status is its last command's
status. Earlier failures are discarded silently, which is the worst possible
failure mode for a gate: the tool prints its findings, the target reports
success, and the job goes green.

These tests drive real GNU make over a scratch Makefile that mirrors the
repository's prologue, so they measure the mechanism rather than describing
it. Each is mutation-proved: the companion test removes the `.SHELLFLAGS`
line and asserts the same probe passes, which is what makes the first test's
verdict meaningful.
"""

from __future__ import annotations

import re
import subprocess  # noqa: S404 - the contract has to run make to observe it.
import typing as typ
from pathlib import Path
from shutil import which

import pytest

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    import collections.abc as cabc

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE = REPOSITORY_ROOT / "Makefile"

SHELL_LINE = re.compile(r"(?m)^SHELL\s*:?=.*$")
SHELLFLAGS_LINE = re.compile(r"(?m)^\.SHELLFLAGS\s*:?=.*$")
ONESHELL_LINE = re.compile(r"(?m)^\.ONESHELL:.*$")

#: A recipe whose first line fails and whose last line succeeds. Under
#: one-shell recipes without `-e` the shell runs both and reports the last
#: one's status, so this target is the smallest thing that tells the two
#: configurations apart.
PROBE_RECIPE = """
probe:
\tfalse
\ttrue
"""


def _make() -> str:
    """Return the absolute path to GNU make.

    Ruff rejects a bare program name in a subprocess call because PATH decides
    what runs, and a readable failure here beats an OSError from deep inside
    the test.
    """
    resolved = which("make")
    assert resolved is not None, "GNU make must be installed to run these contracts"
    return resolved


def _prologue_line(pattern: re.Pattern[str], description: str) -> str:
    """Return the repository Makefile's single line matching ``pattern``."""
    makefile = MAKEFILE.read_text(encoding="utf-8")
    matches = pattern.findall(makefile)
    assert len(matches) == 1, (
        f"expected exactly one {description} line in the Makefile, found {len(matches)}"
    )
    return matches[0]


def _write_scratch_makefile(directory: Path, prologue: cabc.Iterable[str]) -> Path:
    """Write a Makefile carrying ``prologue`` and the probe target."""
    path = directory / "Makefile"
    path.write_text("\n".join([*prologue, PROBE_RECIPE]), encoding="utf-8")
    return path


def _run_probe(makefile: Path) -> subprocess.CompletedProcess[str]:
    """Run the probe target and return the completed process."""
    return subprocess.run(  # noqa: S603 - a resolved make over a scratch file.
        [_make(), "--no-print-directory", "-f", str(makefile), "probe"],
        cwd=makefile.parent,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )


@pytest.fixture
def prologue() -> list[str]:
    """Return the repository Makefile's shell prologue, line for line."""
    return [
        _prologue_line(SHELL_LINE, "SHELL"),
        _prologue_line(SHELLFLAGS_LINE, ".SHELLFLAGS"),
        _prologue_line(ONESHELL_LINE, ".ONESHELL"),
    ]


def test_the_makefile_declares_shellflags_with_abort_on_error() -> None:
    """The prologue must ask the shell to abort on the first failing command.

    This is the cheap half of the contract: it reads the declaration. The
    tests below prove the declaration does what it claims.
    """
    flags = _prologue_line(SHELLFLAGS_LINE, ".SHELLFLAGS")
    _, _, value = flags.partition("=")
    words = value.split()
    assert words, ".SHELLFLAGS must not be empty"
    assert any(
        word.startswith("-") and not word.startswith("--") and "e" in word
        for word in words
    ), f"{flags!r} must pass -e so the shell aborts on the first failure"
    assert any(
        word.startswith("-") and not word.startswith("--") and "c" in word
        for word in words
    ), f"{flags!r} must keep -c; make passes the recipe as a command string"


def test_a_failing_recipe_line_fails_the_target(
    tmp_path: Path, prologue: list[str]
) -> None:
    """A recipe whose first line fails must fail its target.

    The probe's last line succeeds, so a target that passes here is one whose
    status came from the wrong command.
    """
    makefile = _write_scratch_makefile(tmp_path, prologue)

    completed = _run_probe(makefile)

    assert completed.returncode != 0, (
        "the failing first recipe line did not fail the target; "
        f"stdout={completed.stdout!r} stderr={completed.stderr!r}"
    )


def test_the_probe_passes_once_shellflags_is_removed(
    tmp_path: Path, prologue: list[str]
) -> None:
    """Deleting the `.SHELLFLAGS` line must make the same probe pass.

    This is the mutation that gives the test above its meaning. Without it, a
    probe that failed for an unrelated reason, a typo in the recipe or a
    missing shell, would look like proof that the flag works.
    """
    without_flags = [line for line in prologue if not SHELLFLAGS_LINE.match(line)]
    assert len(without_flags) == len(prologue) - 1, (
        "the mutation must remove exactly the .SHELLFLAGS line"
    )
    makefile = _write_scratch_makefile(tmp_path, without_flags)

    completed = _run_probe(makefile)

    assert completed.returncode == 0, (
        "the mutation was expected to restore the swallowed failure, but the "
        f"probe still failed; stderr={completed.stderr!r}"
    )


def test_oneshell_applies_to_targets_it_does_not_name(
    tmp_path: Path, prologue: list[str]
) -> None:
    """`.ONESHELL` is global, which is why the flag is needed at all.

    The repository's declaration names one target. If GNU make ever honoured
    that prerequisite list, one-shell recipes would be scoped and this whole
    contract would be guarding nothing, so the assumption is worth pinning.
    """
    oneshell = _prologue_line(ONESHELL_LINE, ".ONESHELL")
    _, _, named = oneshell.partition(":")
    assert named.split(), (
        "this test assumes .ONESHELL names a target other than 'probe'"
    )
    assert "probe" not in named.split(), "the probe target must not be named"

    without_flags = [line for line in prologue if not SHELLFLAGS_LINE.match(line)]
    makefile = _write_scratch_makefile(tmp_path, without_flags)

    completed = _run_probe(makefile)

    assert completed.returncode == 0, (
        "the probe target ran line by line, so .ONESHELL was scoped to the "
        "target it names and the prologue comment is wrong"
    )
