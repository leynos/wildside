#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts==4.10.1", "cuprum==0.1.0"]
# ///
"""Lint composite actions and workflows, failing on the first bad tool.

This replaces a multi-command shell body in the Makefile's `lint-actions`
recipe. That body ran `action-validator` in a loop over composite actions and
a `yamllint`-then-`actionlint` pair over workflows. In both shapes an earlier
command's failure was replaced by a later one's status, so a real finding
could be reported as a pass. Shell guards can paper over that; the estate's
scripting standards say gate logic of this size belongs in a script instead,
where the ordering is explicit and testable.

Every tool runs in a defined order and the first failure ends the run, naming
the tool and its exit status. A missing `.github/actions` or
`.github/workflows` directory is reported and skipped, which is what the shell
did.

Run it from the repository root:

``uv run scripts/lint_actions.py --yamllint-version 1.35.1``
"""

from __future__ import annotations

import dataclasses as dc
import sys
import typing as typ
from pathlib import Path

import cyclopts
from cuprum import Program, ProgramCatalogue, ProjectSettings, scoped, sh

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    import collections.abc as cabc

#: The only programs this script may run. Cuprum refuses to build a command
#: for anything absent from the catalogue, and `scoped` refuses to execute
#: one, so a typo in a program name fails here rather than reaching a shell.
#:
#: The constructor spelling follows cuprum 0.1.0 rather than the
#: `Catalogue.from_programs` shown in `docs/scripting-standards.md`; that
#: helper does not exist in the published package.
LINT_PROJECT = ProjectSettings(
    name="wildside-lint-actions",
    programs=(Program("uvx"), Program("action-validator"), Program("actionlint")),
    documentation_locations=("docs/developers-guide.md",),
    noise_rules=(),
)
CATALOGUE = ProgramCatalogue(projects=(LINT_PROJECT,))

ACTIONS_DIR = Path(".github/actions")
WORKFLOWS_DIR = Path(".github/workflows")
ACTION_FILENAME = "action.yml"
WORKFLOW_SUFFIXES = (".yml", ".yaml")

app = cyclopts.App(
    name="lint-actions",
    help=(
        "Lint composite actions and workflows with yamllint, action-validator "
        "and actionlint."
    ),
)


class LintError(Exception):
    """A linter rejected its input, or could not be run.

    Carrying the tool's name and status separately keeps the message the same
    shape whether the tool failed or was missing, so a caller never has to
    parse it back out.

    Parameters
    ----------
    tool : str
        Name of the linter, as a reader would refer to it.
    status : int
        Exit status the linter returned.
    detail : str, optional
        Diagnostic text to append to the message.

    Attributes
    ----------
    tool : str
        Name of the linter that failed.
    status : int
        Exit status, reused as the script's own exit status.
    """

    def __init__(self, tool: str, status: int, detail: str = "") -> None:
        """Record which tool failed, with what status, and why."""
        self.tool = tool
        self.status = status
        suffix = f": {detail}" if detail else ""
        super().__init__(f"{tool} failed with exit status {status}{suffix}")


@dc.dataclass(frozen=True, slots=True)
class Invocation:
    """One external command, named by the tool it is really running.

    `uvx` runs yamllint on this project's behalf, so reporting `uvx` as the
    failing tool would send the reader to the wrong place.

    Attributes
    ----------
    tool : str
        Name the failure message uses.
    argv : tuple[str, ...]
        Program and arguments actually executed.
    """

    tool: str
    argv: tuple[str, ...]


def _action_manifests(root: Path) -> list[Path]:
    """Return every composite action manifest, in a stable order."""
    return sorted((root / ACTIONS_DIR).rglob(ACTION_FILENAME))


def _workflow_files(root: Path) -> list[Path]:
    """Return every workflow definition, in a stable order."""
    directory = root / WORKFLOWS_DIR
    return sorted(
        path for suffix in WORKFLOW_SUFFIXES for path in directory.glob(f"*{suffix}")
    )


def _yamllint(version: str, paths: cabc.Sequence[Path]) -> Invocation:
    """Return the pinned yamllint invocation for ``paths``."""
    return Invocation(
        tool="yamllint",
        argv=("uvx", "--from", f"yamllint=={version}", "yamllint", *map(str, paths)),
    )


def plan(root: Path, yamllint_version: str) -> list[Invocation]:
    """Return every invocation the gate will run, in order.

    Building the plan separately from running it is what lets a test assert
    the ordering, and what makes "the first failure wins" a property of one
    loop rather than of control flow scattered through the script.

    Parameters
    ----------
    root : Path
        Repository root to search for actions and workflows.
    yamllint_version : str
        Exact yamllint version the invocations pin.

    Returns
    -------
    list[Invocation]
        Invocations in the order they must run; empty when there is nothing
        to lint.

    Examples
    --------
    >>> plan(Path("/nonexistent"), "1.35.1")
    []
    """
    invocations: list[Invocation] = []

    manifests = _action_manifests(root)
    if manifests:
        invocations.append(_yamllint(yamllint_version, manifests))
        invocations.extend(
            Invocation(
                tool="action-validator", argv=("action-validator", str(manifest))
            )
            for manifest in manifests
        )

    workflows = _workflow_files(root)
    if workflows:
        invocations.extend((
            _yamllint(yamllint_version, workflows),
            Invocation(tool="actionlint", argv=("actionlint", *map(str, workflows))),
        ))

    return invocations


def _run(invocation: Invocation) -> None:
    """Run one invocation, raising `LintError` unless it succeeds.

    `echo=True` mirrors both streams as the tool writes them, which is what a
    gate needs: a linter's findings are the point of running it, and yamllint
    reports warnings even on a successful run. Capture stays on so the failure
    can quote stderr rather than only a status.
    """
    program, *arguments = invocation.argv
    result = sh.make(Program(program), catalogue=CATALOGUE)(*arguments).run_sync(
        capture=True, echo=True
    )
    if result.exit_code != 0:
        raise LintError(
            invocation.tool, result.exit_code, (result.stderr or "").strip()
        )


def lint(root: Path, yamllint_version: str) -> None:
    """Run every planned invocation, stopping at the first failure.

    Parameters
    ----------
    root : Path
        Repository root to lint.
    yamllint_version : str
        Exact yamllint version to run.

    Raises
    ------
    LintError
        If any linter exits non-zero. Later invocations do not run.
    """
    # Flushed, because cuprum's `echo` writes to the file descriptor directly
    # while print buffers: without this the skip notice appears after the
    # output of the tools it precedes.
    if not (root / ACTIONS_DIR).is_dir():
        print("No composite actions found; skipping action lint", flush=True)
    if not (root / WORKFLOWS_DIR).is_dir():
        print("No workflows found; skipping workflow lint", flush=True)

    with scoped(allowlist=frozenset(CATALOGUE.allowlist)):
        for invocation in plan(root, yamllint_version):
            _run(invocation)


@app.default
def main(
    *,
    yamllint_version: str,
    repository: Path = Path(),
) -> None:
    """Lint the repository's composite actions and workflows.

    Parameters
    ----------
    yamllint_version : str
        Exact yamllint version to run, pinned by the caller so the linter
        cannot change underneath the gate.
    repository : Path
        Repository root to lint. Defaults to the working directory.

    Raises
    ------
    SystemExit
        With the failing linter's exit status, after reporting which linter
        failed.
    """
    try:
        lint(repository.resolve(), yamllint_version)
    except LintError as failure:
        print(f"lint-actions: {failure}", file=sys.stderr)
        raise SystemExit(failure.status or 1) from failure


if __name__ == "__main__":  # pragma: no cover - exercised through the CLI.
    app()
