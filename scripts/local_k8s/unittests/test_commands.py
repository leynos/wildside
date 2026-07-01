"""Unit tests for local Kubernetes command execution primitives."""

from __future__ import annotations

import subprocess

import pytest
from plumbum.commands.processes import ProcessExecutionError

from local_k8s.commands import CommandResult, run
from local_k8s.validation import LocalK8sError


def test_run_sends_input_text_through_subprocess(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify stdin text is routed through the subprocess runner."""
    calls: list[tuple[str, list[str], str | None, str]] = []

    def record_run_with_input(
        command: str,
        args: list[str],
        *,
        cwd: str | None = None,
        input_text: str,
    ) -> CommandResult:
        calls.append((command, args, cwd, input_text))
        return CommandResult(stdout="created\n", stderr="")

    monkeypatch.setattr("local_k8s.commands._run_with_input", record_run_with_input)

    result = run("kind", ["create", "cluster", "--config", "-"], cwd="/repo", input_text="kind: Cluster\n")

    assert result == CommandResult(stdout="created\n", stderr=""), (
        "run must return the subprocess result unchanged"
    )
    assert calls == [
        (
            "kind",
            ["create", "cluster", "--config", "-"],
            "/repo",
            "kind: Cluster\n",
        )
    ], "run must forward command arguments, cwd, and stdin to the subprocess runner"


def test_run_wraps_subprocess_failures(monkeypatch: pytest.MonkeyPatch) -> None:
    """Verify subprocess failures are normalized to LocalK8sError."""

    def fail_with_called_process_error(
        _command: str,
        _args: list[str],
        *,
        cwd: str | None = None,
        input_text: str,
    ) -> CommandResult:
        assert cwd is None, "subprocess runner must receive the default working directory"
        assert input_text == "invalid", "subprocess runner must receive the provided stdin text"
        raise subprocess.CalledProcessError(
            1,
            ["kind", "create", "cluster"],
            stderr="kind rejected config\n",
        )

    monkeypatch.setattr("local_k8s.commands._run_with_input", fail_with_called_process_error)

    with pytest.raises(LocalK8sError, match="kind rejected config"):
        run("kind", ["create", "cluster"], input_text="invalid")


def test_run_returns_plumbum_results(monkeypatch: pytest.MonkeyPatch) -> None:
    """Verify successful plumbum execution is returned unchanged."""
    expected = CommandResult(stdout="release ok\n", stderr="")
    calls: list[tuple[str, list[str], str | None]] = []

    def record_run_with_plumbum(
        command: str,
        args: list[str],
        *,
        cwd: str | None = None,
    ) -> CommandResult:
        calls.append((command, args, cwd))
        return expected

    monkeypatch.setattr("local_k8s.commands._run_with_plumbum", record_run_with_plumbum)

    result = run("helm", ["status", "wildside"], cwd="/repo", input_text=None)

    assert result is expected, "run must return the plumbum result unchanged"
    assert calls == [
        ("helm", ["status", "wildside"], "/repo")
    ], "run must forward command arguments and cwd to the plumbum runner"


def test_run_wraps_plumbum_failures(monkeypatch: pytest.MonkeyPatch) -> None:
    """Verify plumbum failures are normalized to LocalK8sError."""

    def fail_with_process_execution_error(
        _command: str,
        _args: list[str],
        *,
        cwd: str | None = None,
    ) -> CommandResult:
        assert cwd is None, "plumbum runner must receive the default working directory"
        raise ProcessExecutionError(["helm", "status"], 1, "", "release missing\n")

    monkeypatch.setattr("local_k8s.commands._run_with_plumbum", fail_with_process_execution_error)

    with pytest.raises(LocalK8sError, match="release missing"):
        run("helm", ["status"])
