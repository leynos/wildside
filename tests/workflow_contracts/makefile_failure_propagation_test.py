"""Contract tests proving a failing recipe command fails its Make target.

`.ONESHELL` is a global special target: GNU make ignores its prerequisite
list, so naming a single target enables one-shell recipes for the whole file.
Every multi-line recipe then reaches the shell as one script, and under make's
default `.SHELLFLAGS` of `-c` that script's status is its last command's
status. Earlier failures are discarded silently, which is the worst failure
mode a gate can have: the tool prints its findings, the target reports
success, and the job goes green.

Two options are needed and neither covers the other. `-e` aborts at the first
failing command, catching a tool that is not the recipe's last line.
`-o pipefail` gives a pipeline its first failing stage's status, catching a
failure at a pipeline's head. This Makefile pipes into the tool that does the
checking, so a dead head yields an empty list and a gate that passes having
examined nothing.

These tests copy the repository's real `SHELL`, `.SHELLFLAGS` and `.ONESHELL`
lines into a scratch Makefile and drive GNU make over two probe recipes, so
they measure the mechanism rather than describing it. Each probe is
mutation-proved against the two half-measures: `-ec` alone lets the
pipeline-head probe pass, and `-o pipefail -c` alone lets the earlier-line
probe pass.
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

#: Two recipes that a correctly configured shell must fail and a default one
#: reports as successful.
#:
#: `earlier-line` fails on a line that is not the last, which only `-e`
#: catches. `pipeline-head` fails in a pipeline's first stage while the
#: pipeline's last stage succeeds, which only `-o pipefail` catches. Together
#: they separate the two options, so neither can be dropped without a test
#: going red.
PROBE_RECIPES = """
earlier-line:
\tfalse
\ttrue

pipeline-head:
\tfalse | cat
"""

#: The half-measures this contract exists to rule out, each with the probe it
#: fails to catch.
HALF_MEASURES = (
    pytest.param(".SHELLFLAGS := -ec", "pipeline-head", id="abort-without-pipefail"),
    pytest.param(
        ".SHELLFLAGS := -o pipefail -c", "earlier-line", id="pipefail-without-abort"
    ),
)


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
    """Write a Makefile carrying ``prologue`` and both probe targets."""
    path = directory / "Makefile"
    path.write_text("\n".join([*prologue, PROBE_RECIPES]), encoding="utf-8")
    return path


def _run_probe(makefile: Path, target: str) -> subprocess.CompletedProcess[str]:
    """Run one probe target and return the completed process."""
    return subprocess.run(  # noqa: S603 - a resolved make over a scratch file.
        [_make(), "--no-print-directory", "-f", str(makefile), target],
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


def test_the_copied_prologue_still_declares_oneshell(prologue: list[str]) -> None:
    """The scratch Makefile must inherit `.ONESHELL`, or it proves nothing.

    Without one-shell recipes make runs each line in its own shell and catches
    the earlier-line probe unaided, so the probes below would pass with any
    `.SHELLFLAGS` at all. This asserts the copied prologue carries the
    declaration, and that it names a target other than the probes, which is
    the global behaviour the whole contract rests on.
    """
    declarations = [line for line in prologue if ONESHELL_LINE.match(line)]
    assert len(declarations) == 1, (
        "the copied prologue must carry exactly one .ONESHELL declaration, "
        f"found {len(declarations)}"
    )
    _, _, named = declarations[0].partition(":")
    assert named.split(), (
        ".ONESHELL must name a target, so the probes exercise its global reach"
    )
    assert not {"earlier-line", "pipeline-head"} & set(named.split()), (
        "the probe targets must not be named; .ONESHELL has to reach them "
        "without being asked"
    )


def test_the_makefile_asks_the_shell_to_abort_and_to_fail_pipelines() -> None:
    """The prologue must pass `-e`, `-o pipefail` and `-c`.

    This is the cheap half of the contract: it reads the declaration. The
    tests below prove the declaration does what it claims.
    """
    flags = _prologue_line(SHELLFLAGS_LINE, ".SHELLFLAGS")
    _, _, value = flags.partition("=")
    words = value.split()
    assert words, ".SHELLFLAGS must not be empty"
    short = [
        word for word in words if word.startswith("-") and not word.startswith("--")
    ]
    assert any("e" in word for word in short), (
        f"{flags!r} must pass -e so the shell aborts on the first failure"
    )
    assert "pipefail" in words, (
        f"{flags!r} must pass -o pipefail; without it a failure at a "
        "pipeline's head is discarded"
    )
    assert words[-1].endswith("c"), (
        f"{flags!r} must end with -c; make appends the recipe as a command string"
    )


@pytest.mark.parametrize("target", ["earlier-line", "pipeline-head"])
def test_a_failing_recipe_command_fails_the_target(
    tmp_path: Path, prologue: list[str], target: str
) -> None:
    """Each probe's failing command must fail its target.

    Both probes end on a command that succeeds, so a target that passes here
    is one whose status came from the wrong command.
    """
    makefile = _write_scratch_makefile(tmp_path, prologue)

    completed = _run_probe(makefile, target)

    assert completed.returncode != 0, (
        f"the failing command in {target} did not fail the target; "
        f"stdout={completed.stdout!r} stderr={completed.stderr!r}"
    )


@pytest.mark.parametrize(("flags", "masked_target"), HALF_MEASURES)
def test_each_half_measure_masks_one_probe(
    tmp_path: Path, prologue: list[str], flags: str, masked_target: str
) -> None:
    """Neither option alone catches both probes.

    This is the mutation that gives the test above its meaning. Substituting
    each half-measure for the real `.SHELLFLAGS` line must let its
    corresponding probe pass, which is what proves the probe is measuring that
    option rather than failing for an unrelated reason, and what stops anyone
    simplifying the flag to one option later.
    """
    weakened = [flags if SHELLFLAGS_LINE.match(line) else line for line in prologue]
    assert flags in weakened, "the mutation must replace the .SHELLFLAGS line"
    makefile = _write_scratch_makefile(tmp_path, weakened)

    completed = _run_probe(makefile, masked_target)

    assert completed.returncode == 0, (
        f"{flags!r} was expected to mask {masked_target}, but the probe still "
        f"failed; stderr={completed.stderr!r}"
    )


def test_removing_shellflags_masks_the_earlier_line_probe(
    tmp_path: Path, prologue: list[str]
) -> None:
    """Dropping the declaration entirely restores make's default behaviour.

    `-c` is what make uses when `.SHELLFLAGS` is absent, so this pins the
    baseline the fix departs from rather than assuming it.
    """
    without_flags = [line for line in prologue if not SHELLFLAGS_LINE.match(line)]
    assert len(without_flags) == len(prologue) - 1, (
        "the mutation must remove exactly the .SHELLFLAGS line"
    )
    makefile = _write_scratch_makefile(tmp_path, without_flags)

    completed = _run_probe(makefile, "earlier-line")

    assert completed.returncode == 0, (
        "make's default .SHELLFLAGS was expected to discard the earlier "
        f"failure; stderr={completed.stderr!r}"
    )
