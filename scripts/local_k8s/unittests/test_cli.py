"""Smoke tests for the local Kubernetes preview CLI boundary."""

from __future__ import annotations

import json
import os
import subprocess
import textwrap
from collections.abc import Callable
from pathlib import Path
from shutil import which
from typing import cast


def test_local_k8s_cli_help_smoke(uv_executable: str, local_k8s_script: Path) -> None:
    """Verify the script entry point loads and exposes the preview CLI."""
    completed = subprocess.run(  # noqa: S603 - argv is fixed by the test.
        [uv_executable, "run", str(local_k8s_script), "--help"],
        text=True,
        capture_output=True,
        check=True,
        timeout=60,
    )

    assert (
        "Manage a local Kubernetes Wildside preview environment." in completed.stdout
    ), "local_k8s.py --help must return the preview CLI help text"


def test_local_k8s_status_reports_configuration_errors_at_cli_boundary(
    uv_executable: str,
    local_k8s_script: Path,
) -> None:
    """Verify workflow commands surface validation failures through the CLI."""
    env = os.environ.copy()
    env["WILDSIDE_K8S_CLUSTER"] = "../wildside"

    completed = subprocess.run(  # noqa: S603 - argv is fixed by the test.
        [uv_executable, "run", str(local_k8s_script), "status"],
        text=True,
        capture_output=True,
        check=False,
        env=env,
        timeout=60,
    )

    assert completed.returncode != 0, (
        "invalid configuration must make the CLI return a nonzero status"
    )
    assert "local preview status failed:" in completed.stderr, (
        "CLI boundary must include the workflow failure prefix"
    )
    assert "WILDSIDE_K8S_CLUSTER" in completed.stderr, (
        "CLI boundary must surface the invalid environment variable name"
    )


def _write_fake_tool(fake_bin: Path) -> None:
    """Write fake preview executables used by the Makefile smoke test."""
    fake_tool = fake_bin / "fake_tool.py"
    fake_tool.write_text(
        textwrap.dedent(
            """\
            #!/usr/bin/env python3
            from __future__ import annotations

            import json
            import os
            import sys
            from pathlib import Path

            name = Path(sys.argv[0]).name
            args = sys.argv[1:]
            state_path = Path(os.environ["WILDSIDE_FAKE_TOOL_STATE"])
            log_path = Path(os.environ["WILDSIDE_FAKE_TOOL_LOG"])
            stdin_text = sys.stdin.read()
            log_path.write_text(
                log_path.read_text() + json.dumps([name, args, bool(stdin_text)]) + "\\n"
                if log_path.exists()
                else json.dumps([name, args, bool(stdin_text)]) + "\\n"
            )

            def has_cluster() -> bool:
                return state_path.exists() and state_path.read_text() == "created"

            if name == "k3d" and args[:3] == ["cluster", "list", "--output"]:
                print('[{"name":"wildside-preview"}]' if has_cluster() else "[]")
            elif name == "k3d" and args[:2] == ["cluster", "create"]:
                state_path.write_text("created")
            elif name == "k3d" and args[:2] == ["cluster", "delete"]:
                state_path.unlink(missing_ok=True)
            elif name == "helm" and args[:2] == ["--kube-context", "k3d-wildside-preview"]:
                print("helm status")
            elif name == "kubectl" and "logs" in args:
                print("backend log")
            elif name == "kubectl" and "get" in args and "pods" in args:
                print("pod/wildside-backend Running")
            elif name == "kubectl" and "get" in args and "service" in args:
                print("service/wildside")
            """
        ),
        encoding="utf8",
    )
    fake_tool.chmod(0o755)
    for tool_name in ("docker", "helm", "k3d", "kubectl"):
        (fake_bin / tool_name).symlink_to(fake_tool)


def _run_make_targets(env: dict[str, str], targets: tuple[str, ...]) -> None:
    """Run preview Makefile targets through the real CLI boundary."""
    make = which("make")
    assert make is not None, "make must be available to execute preview targets"
    for target in targets:
        completed = subprocess.run(  # noqa: S603 - argv is fixed by the test.
            [make, "--no-print-directory", target],
            text=True,
            capture_output=True,
            check=False,
            env=env,
            timeout=120,
        )
        assert completed.returncode == 0, (
            f"{target} should complete through the local preview CLI; "
            f"stdout={completed.stdout!r} stderr={completed.stderr!r}"
        )


def _load_log_entries(log_path: Path) -> list[list[object]]:
    """Load fake tool command records from the JSON-lines log."""
    return [
        json.loads(line) for line in log_path.read_text(encoding="utf8").splitlines()
    ]


def _assert_command_logged(
    log_entries: list[list[object]],
    tool: str,
    predicate: Callable[[list[object]], bool],
    message: str,
) -> None:
    """Assert a fake-tool log contains a matching command."""
    assert any(
        entry[0] == tool and predicate(cast(list[object], entry[1]))
        for entry in log_entries
    ), f"{message}; recorded commands: {log_entries!r}"


def test_local_k8s_make_targets_smoke_successful_flow(tmp_path: Path) -> None:
    """Verify Makefile preview targets cross the real CLI boundary."""
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _write_fake_tool(fake_bin)

    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
    env["WILDSIDE_FAKE_TOOL_LOG"] = str(tmp_path / "commands.jsonl")
    env["WILDSIDE_FAKE_TOOL_STATE"] = str(tmp_path / "cluster-state")

    _run_make_targets(
        env, ("local-k8s-up", "local-k8s-status", "local-k8s-logs", "local-k8s-down")
    )

    log_entries = _load_log_entries(Path(env["WILDSIDE_FAKE_TOOL_LOG"]))
    _assert_command_logged(
        log_entries,
        "docker",
        lambda args: args[0] == "build",
        "local-k8s-up must build the backend image through the CLI boundary",
    )
    _assert_command_logged(
        log_entries,
        "helm",
        lambda args: "status" in args,
        "local-k8s-status must inspect the Helm release through the CLI boundary",
    )
    _assert_command_logged(
        log_entries,
        "kubectl",
        lambda args: "logs" in args,
        "local-k8s-logs must stream pod logs through the CLI boundary",
    )
    assert not Path(env["WILDSIDE_FAKE_TOOL_STATE"]).exists(), (
        "local-k8s-down must delete the preview cluster through the CLI boundary"
    )
