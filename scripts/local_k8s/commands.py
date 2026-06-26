"""Command execution primitives for the local preview workflow.

Runs local tooling commands (Docker, Helm, k3d, kubectl) via plumbum and
normalizes execution failures into `LocalK8sError` for use by CLI workflows.

Examples
--------
>>> from local_k8s.commands import run
>>> result = run("kubectl", ["version", "--client"])
>>> print(result.stdout)
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
import logging
import subprocess

from plumbum import local
from plumbum.commands.processes import ProcessExecutionError

from .validation import LocalK8sError

logger = logging.getLogger(__name__)


@dataclass(frozen=True, slots=True)
class CommandResult:
    """Captured output from an external command.

    Attributes
    ----------
    stdout : str
        Captured standard output stream.
    stderr : str
        Captured standard error stream.
    """

    stdout: str
    stderr: str


def _run_with_input(
    command: str,
    args: Sequence[str],
    *,
    cwd: str | None = None,
    input_text: str,
) -> CommandResult:
    """Run a command with stdin text via subprocess."""
    completed = subprocess.run(  # noqa: S603 - command is built internally.
        [command, *args],
        input=input_text,
        text=True,
        capture_output=True,
        check=True,
        cwd=cwd,
    )
    return CommandResult(stdout=completed.stdout, stderr=completed.stderr)


def _run_with_plumbum(
    command: str,
    args: Sequence[str],
    *,
    cwd: str | None = None,
) -> CommandResult:
    """Run a command through plumbum and capture output."""
    executable = local[command]
    if cwd:
        with local.cwd(cwd):
            out = executable.run(args)
    else:
        out = executable.run(args)
    return CommandResult(stdout=out[1], stderr=out[2])


def _command_error_message(
    exc: ProcessExecutionError | subprocess.CalledProcessError,
) -> str:
    """Return a normalized message for command execution failures."""
    stderr = exc.stderr or ""
    return stderr.strip() or str(exc)


def run(
    command: str,
    args: Sequence[str],
    *,
    cwd: str | None = None,
    input_text: str | None = None,
) -> CommandResult:
    """Run a local command and capture its output.

    Parameters
    ----------
    command : str
        Executable name resolved from ``PATH``.
    args : Sequence[str]
        Positional arguments forwarded to the executable.
    cwd : str | None, optional
        Working directory for execution. Uses the process current directory
        when unset.
    input_text : str | None, optional
        Text sent to standard input. Used for commands such as ``kind create
        cluster --config -``.

    Returns
    -------
    CommandResult
        Captured stdout and stderr from the completed process.

    Raises
    ------
    LocalK8sError
        Raised when the process exits with a non-zero status.
    """

    try:
        if input_text is not None:
            return _run_with_input(command, args, cwd=cwd, input_text=input_text)
        return _run_with_plumbum(command, args, cwd=cwd)
    except (ProcessExecutionError, subprocess.CalledProcessError) as exc:
        logger.error(
            "local_k8s_command_failed",
            extra={
                "command": command,
                "failure_category": type(exc).__name__,
            },
        )
        raise LocalK8sError(_command_error_message(exc)) from exc
