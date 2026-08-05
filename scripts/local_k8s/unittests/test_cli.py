"""Smoke tests for the local Kubernetes preview CLI boundary."""

from __future__ import annotations

import json
import os
import subprocess  # noqa: S404 - tests deliberately exercise the CLI via subprocess.
import sys
import typing as typ
from pathlib import Path
from shutil import which

import pytest

if typ.TYPE_CHECKING:
    import collections.abc as cabc

FAKE_TOOL_SOURCE = (
    Path(__file__)
    .with_name("fixtures")
    .joinpath("fake_preview_tool.py.txt")
    .read_text(encoding="utf8")
)

FAKE_TOOL_NAMES = (
    "docker",
    "podman",
    "helm",
    "k3d",
    "kind",
    "kubectl",
    "uv",
    "env",
    "systemd-run",
)


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
    fake_tool.write_text(FAKE_TOOL_SOURCE, encoding="utf8")
    fake_tool.chmod(0o755)
    for tool_name in FAKE_TOOL_NAMES:
        (fake_bin / tool_name).symlink_to(fake_tool)


def _run_make_targets(env: dict[str, str], targets: tuple[str, ...]) -> None:
    """Run preview Makefile targets through the real CLI boundary."""
    make = which("make")
    assert make is not None, "make must be available to execute preview targets"
    for target in targets:
        completed = subprocess.run(  # noqa: S603 - argv is fixed by the test.
            [make, "--no-print-directory", f"PATH={env['PATH']}", target],
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
    entries: list[list[object]] = []
    for line_number, line in enumerate(
        log_path.read_text(encoding="utf8").splitlines(), start=1
    ):
        decoded = json.loads(line)
        is_command_record = (
            isinstance(decoded, list)
            and len(decoded) == 3
            and isinstance(decoded[0], str)
            and isinstance(decoded[1], list)
            and all(isinstance(argument, str) for argument in decoded[1])
            and isinstance(decoded[2], bool)
        )
        assert is_command_record, (
            f"fake tool log line {line_number} must decode to "
            f"[str, list[str], bool], got {type(decoded).__name__}: {decoded!r}"
        )
        entries.append(decoded)
    return entries


@pytest.mark.parametrize(
    ("source", "expected", "expected_error", "message"),
    [
        pytest.param(
            '["docker", ["build"], false]\n["helm", ["status"], true]\n',
            [["docker", ["build"], False], ["helm", ["status"], True]],
            None,
            None,
            id="multiple-lists",
        ),
        pytest.param("[]\n", None, AssertionError, "line 1.*list", id="wrong-length"),
        pytest.param(
            '[1, ["build"], false]\n',
            None,
            AssertionError,
            "line 1.*list",
            id="non-string-tool",
        ),
        pytest.param(
            '["docker", "build", false]\n',
            None,
            AssertionError,
            "line 1.*list",
            id="non-list-arguments",
        ),
        pytest.param(
            '["docker", [1], false]\n',
            None,
            AssertionError,
            "line 1.*list",
            id="non-string-argument",
        ),
        pytest.param(
            '["docker", ["build"], 0]\n',
            None,
            AssertionError,
            "line 1.*list",
            id="non-boolean-stdin",
        ),
        pytest.param(
            '{"tool": "docker"}\n', None, AssertionError, "line 1.*dict", id="object"
        ),
        pytest.param("42\n", None, AssertionError, "line 1.*int", id="scalar"),
        pytest.param("{invalid\n", None, json.JSONDecodeError, None, id="invalid-json"),
    ],
)
def test_load_log_entries_validates_each_json_line(
    tmp_path: Path,
    source: str,
    expected: list[list[object]] | None,
    expected_error: type[Exception] | None,
    message: str | None,
) -> None:
    """Return list entries and reject other decoded JSON shapes."""
    log_path = tmp_path / "commands.jsonl"
    log_path.write_text(source, encoding="utf8")

    if expected_error is None:
        assert _load_log_entries(log_path) == expected, (
            "decoded fake-tool records must match the complete command log"
        )
    else:
        with pytest.raises(expected_error, match=message):
            _load_log_entries(log_path)


def _assert_command_logged(
    log_entries: list[list[object]],
    tool: str,
    # Sequence, not list: `list` is invariant, so a narrowed `list[Unknown]`
    # read back out of a log entry does not satisfy `list[object]`.
    predicate: cabc.Callable[[cabc.Sequence[object]], bool],
    message: str,
) -> None:
    """Assert a fake-tool log contains a matching command."""
    for entry in log_entries:
        arguments = entry[1]
        assert isinstance(arguments, list), "validated command arguments must be a list"
        if entry[0] == tool and predicate(arguments):
            return
    failure = f"{message}; recorded commands: {log_entries!r}"
    raise AssertionError(failure)


def test_fake_tool_rejects_unsupported_wrapper_options(tmp_path: Path) -> None:
    """Reject wrapper options outside the production command contract."""
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _write_fake_tool(fake_bin)
    env = os.environ | {
        "WILDSIDE_FAKE_TOOL_LOG": str(tmp_path / "commands.jsonl"),
        "WILDSIDE_FAKE_TOOL_STATE": str(tmp_path / "cluster-state"),
        "WILDSIDE_CONTAINER_ENGINE": "podman",
    }

    completed = subprocess.run(  # noqa: S603 - executable is the test fake.
        [fake_bin / "env", "--unexpected", "kind", "get", "clusters"],
        text=True,
        capture_output=True,
        check=False,
        env=env,
    )

    assert completed.returncode != 0, (
        "unsupported wrapper options must preserve fail-closed fake dispatch"
    )
    assert "unexpected fake command: env" in completed.stderr, (
        "fake dispatch must identify the rejected wrapper command"
    )


@pytest.mark.parametrize(
    ("container_engine", "k8s_provider"),
    [
        pytest.param("docker", "k3d", id="docker-k3d"),
        pytest.param("podman", "kind", id="podman-kind"),
    ],
)
def test_local_k8s_make_targets_smoke_successful_flow(
    tmp_path: Path,
    container_engine: str,
    k8s_provider: str,
) -> None:
    """Verify Makefile preview targets cross the real CLI boundary."""
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _write_fake_tool(fake_bin)

    env = os.environ.copy()
    # A host BASH_ENV can rewrite PATH when Make starts its recipe shell.
    env.pop("BASH_ENV", None)
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
    env["UV"] = str(fake_bin / "uv")
    env["WILDSIDE_FAKE_PYTHON"] = sys.executable
    env["WILDSIDE_FAKE_TOOL_LOG"] = str(tmp_path / "commands.jsonl")
    env["WILDSIDE_FAKE_TOOL_STATE"] = str(tmp_path / "cluster-state")
    env["WILDSIDE_CONTAINER_ENGINE"] = container_engine
    env["WILDSIDE_K8S_PROVIDER"] = k8s_provider

    state_path = Path(env["WILDSIDE_FAKE_TOOL_STATE"])

    _run_make_targets(env, ("local-k8s-up",))
    assert state_path.exists(), (
        "local-k8s-up must create the preview cluster through the CLI boundary"
    )
    assert state_path.read_text(encoding="utf8") == "created", (
        "local-k8s-up must record the created-cluster marker the fake tool's "
        "has_cluster() check reads"
    )

    _run_make_targets(env, ("local-k8s-status", "local-k8s-logs"))

    log_entries = _load_log_entries(Path(env["WILDSIDE_FAKE_TOOL_LOG"]))
    _assert_command_logged(
        log_entries,
        container_engine,
        lambda args: args[0] == "build",
        "local-k8s-up must build the backend image through the CLI boundary",
    )
    if k8s_provider == "k3d":
        _assert_command_logged(
            log_entries,
            "k3d",
            lambda args: (
                args[:2] == ["image", "import"]
                and args[-2:] == ["--cluster", "wildside-preview"]
            ),
            "local-k8s-up must import the image into the k3d cluster",
        )
    elif container_engine == "podman":
        _assert_command_logged(
            log_entries,
            "podman",
            lambda args: args[:2] == ["save", "--output"],
            "local-k8s-up must save the Podman image for kind",
        )
        _assert_command_logged(
            log_entries,
            "env",
            lambda args: (
                args[:4]
                == [
                    "KIND_EXPERIMENTAL_PROVIDER=podman",
                    "kind",
                    "load",
                    "image-archive",
                ]
                and args[-2:] == ["--name", "wildside-preview"]
            ),
            "local-k8s-up must load the Podman image archive into kind",
        )
    _assert_command_logged(
        log_entries,
        "helm",
        lambda args: "upgrade" in args and "--install" in args,
        "local-k8s-up must install or upgrade the Helm release",
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

    _run_make_targets(env, ("local-k8s-down",))
    assert not state_path.exists(), (
        "local-k8s-down must delete the preview cluster through the CLI boundary"
    )
    unexpected = subprocess.run(  # noqa: S603 - executable is the test fake.
        [fake_bin / container_engine, "push", "wildside-backend:local"],
        text=True,
        capture_output=True,
        check=False,
        env=env,
    )
    assert unexpected.returncode != 0, (
        "an unmodelled container-engine command must fail closed"
    )
    assert f"unexpected fake command: {container_engine}" in unexpected.stderr, (
        f"stderr must identify the rejected {container_engine} command"
    )
