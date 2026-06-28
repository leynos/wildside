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

    assert result == CommandResult(stdout="created\n", stderr="")
    assert calls == [
        (
            "kind",
            ["create", "cluster", "--config", "-"],
            "/repo",
            "kind: Cluster\n",
        )
    ]


def test_run_wraps_subprocess_failures(monkeypatch: pytest.MonkeyPatch) -> None:
    """Verify subprocess failures are normalized to LocalK8sError."""

    def fail_with_called_process_error(
        _command: str,
        _args: list[str],
        *,
        cwd: str | None = None,
        input_text: str,
    ) -> CommandResult:
        assert cwd is None
        assert input_text == "invalid"
        raise subprocess.CalledProcessError(
            1,
            ["kind", "create", "cluster"],
            stderr="kind rejected config\n",
        )

    monkeypatch.setattr("local_k8s.commands._run_with_input", fail_with_called_process_error)

    with pytest.raises(LocalK8sError, match="kind rejected config"):
        run("kind", ["create", "cluster"], input_text="invalid")


def test_run_wraps_plumbum_failures(monkeypatch: pytest.MonkeyPatch) -> None:
    """Verify plumbum failures are normalized to LocalK8sError."""

    def fail_with_process_execution_error(
        _command: str,
        _args: list[str],
        *,
        cwd: str | None = None,
    ) -> CommandResult:
        assert cwd is None
        raise ProcessExecutionError(["helm", "status"], 1, "", "release missing\n")

    monkeypatch.setattr("local_k8s.commands._run_with_plumbum", fail_with_process_execution_error)

    with pytest.raises(LocalK8sError, match="release missing"):
        run("helm", ["status"])
