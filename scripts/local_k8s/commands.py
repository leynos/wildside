"""Command execution primitives for the local preview workflow."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass

from plumbum import local
from plumbum.commands.processes import ProcessExecutionError

from .validation import LocalK8sError


@dataclass(frozen=True, slots=True)
class CommandResult:
    """Captured stdout and stderr from an external command."""

    stdout: str
    stderr: str


def run(command: str, args: Sequence[str], *, cwd: str | None = None) -> CommandResult:
    """Run a command and raise a local preview error on failure."""

    try:
        executable = local[command]
        if cwd:
            with local.cwd(cwd):
                out = executable.run(args)
        else:
            out = executable.run(args)
    except ProcessExecutionError as exc:
        raise LocalK8sError(exc.stderr.strip() or str(exc)) from exc
    return CommandResult(stdout=out[1], stderr=out[2])
