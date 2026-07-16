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
from typing import NoReturn

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
    """Run a command through plumbum and capture output.

    Binds the working directory to the command with ``with_cwd`` rather than the
    process-wide ``local.cwd`` context manager, so concurrent ``run(..., cwd=)``
    calls do not race on the global current directory.
    """
    executable = local[command]
    if cwd:
        executable = executable.with_cwd(cwd)
    out = executable.run(args)
    return CommandResult(stdout=out[1], stderr=out[2])


def _run_streaming_with_subprocess(
    command: str,
    args: Sequence[str],
    *,
    cwd: str | None = None,
) -> None:
    """Run a command with inherited stdout and stderr."""
    subprocess.run(  # noqa: S603, S607 - local preview tools are PATH-resolved.
        [command, *args],
        check=True,
        cwd=cwd,
    )


def _command_error_message(
    exc: ProcessExecutionError | subprocess.CalledProcessError,
) -> str:
    """Return a normalized message for command execution failures."""
    stderr = exc.stderr or ""
    return stderr.strip() or str(exc)


def _command_return_code(
    exc: ProcessExecutionError | subprocess.CalledProcessError,
) -> int | None:
    """Return the process exit status from either failure type."""
    # ProcessExecutionError exposes ``retcode``; CalledProcessError uses
    # ``returncode``. Prefer whichever attribute the exception carries.
    code = getattr(exc, "returncode", None)
    if code is None:
        code = getattr(exc, "retcode", None)
    return code


def _log_and_raise(
    command: str,
    exc: ProcessExecutionError | subprocess.CalledProcessError,
) -> NoReturn:
    """Log a command failure and raise the local preview error wrapper."""
    logger.exception(
        "local_k8s_command_failed",
        extra={
            "command": command,
            "failure_category": type(exc).__name__,
        },
    )
    raise LocalK8sError(
        _command_error_message(exc),
        stderr=exc.stderr,
        returncode=_command_return_code(exc),
    ) from exc


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

    Examples
    --------
    >>> result = run("kubectl", ["version", "--client"])  # doctest: +SKIP
    >>> print(result.stdout)  # doctest: +SKIP
    >>> run("kind", ["create", "cluster", "--config", "-"],
    ...     input_text=config_yaml)  # doctest: +SKIP
    """

    try:
        if input_text is not None:
            return _run_with_input(command, args, cwd=cwd, input_text=input_text)
        return _run_with_plumbum(command, args, cwd=cwd)
    except (ProcessExecutionError, subprocess.CalledProcessError) as exc:
        _log_and_raise(command, exc)


def run_streaming(
    command: str,
    args: Sequence[str],
    *,
    cwd: str | None = None,
) -> None:
    """Run a local command while inheriting stdout and stderr.

    Parameters
    ----------
    command : str
        Executable name resolved from ``PATH``.
    args : Sequence[str]
        Positional arguments forwarded to the executable.
    cwd : str | None, optional
        Working directory for execution. Uses the process current directory
        when unset.

    Raises
    ------
    LocalK8sError
        Raised when the process exits with a non-zero status.

    Examples
    --------
    >>> run_streaming("kubectl", ["logs", "-f", "deploy/backend"])  # doctest: +SKIP
    """

    try:
        _run_streaming_with_subprocess(command, args, cwd=cwd)
    except subprocess.CalledProcessError as exc:
        _log_and_raise(command, exc)
