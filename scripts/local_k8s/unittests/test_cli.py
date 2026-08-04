"""Smoke tests for the local Kubernetes preview CLI boundary."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import textwrap
from collections.abc import Callable
from pathlib import Path
from shutil import which
from typing import cast

import pytest

FAKE_TOOL_SOURCE = textwrap.dedent(
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
    with log_path.open("a", encoding="utf8") as log_file:
        print(json.dumps([name, args, bool(stdin_text)]), file=log_file)
    def _unwrap(name: str, args: list[str]) -> tuple[str, list[str]]:
        # Emulate `systemd-run --scope --user -p KEY=VAL env VAR=x kind ...`
        # and `env VAR=x kind ...` by stripping scope flags and leading
        # `VAR=value` assignments, then re-dispatching to the wrapped tool.
        while name in ("env", "systemd-run"):
            rest = list(args)
            while rest and (rest[0].startswith("-") or "=" in rest[0] or rest[0] == "env"):
                if rest[0] == "-p":
                    rest = rest[2:]
                else:
                    rest = rest[1:]
            if not rest:
                return name, args
            name, args = rest[0], rest[1:]
        return name, args
    name, args = _unwrap(name, args)
    if name == "uv" and args[:2] == ["run", "scripts/local_k8s.py"] and args[2:] in (
        ["up"], ["status"], ["logs"], ["down"],
    ):
        python = os.environ["WILDSIDE_FAKE_PYTHON"]
        os.execv(python, [str(python), *args[1:]])
    def has_cluster() -> bool:
        return state_path.exists() and state_path.read_text() == "created"
    cluster_name = "wildside-preview"
    contexts = ("k3d-wildside-preview", "kind-wildside-preview")
    def contextual(flag: str, suffix: list[str]) -> bool:
        return len(args) >= 2 and args[0] == flag and args[1] in contexts and args[2:] == suffix
    is_build = (
        name == os.environ["WILDSIDE_CONTAINER_ENGINE"]
        and len(args) == 6 and args[:2] == ["build", "-f"]
        and args[3:5] == ["-t", "wildside-backend:local"]
    )
    is_podman_save = (
        name == "podman" and len(args) == 4 and args[:2] == ["save", "--output"]
        and args[3] == "docker.io/library/wildside-backend:local"
    )
    is_kind_load = (
        name == "kind" and len(args) == 5 and args[:2] == ["load", "image-archive"]
        and args[3:] == ["--name", cluster_name]
    )
    is_helm_upgrade = (
        name == "helm" and len(args) == 17
        and args[:1] == ["--kube-context"] and args[1] in contexts
        and args[2:5] == ["upgrade", "--install", "wildside"]
        and args[6:8] == ["--namespace", "wildside"] and args[8] == "--values"
        and args[10:] == [
            "--set-string", "image.repository=wildside-backend",
            "--set-string", "image.tag=local",
            "--wait", "--timeout", "5m",
        ]
    )
    is_session_secret_get = name == "kubectl" and contextual(
        "--context", ["-n", "wildside", "get", "secret", "wildside-session-key", "--ignore-not-found",
        "-o=jsonpath={.data.session_key}"],
    )
    is_session_secret_create = (
        name == "kubectl" and contextual("--context", ["create", "-f", "-"]) and bool(stdin_text)
    )
    is_no_output_command = any((
        is_build,
        name == "k3d" and args == ["image", "import", "wildside-backend:local", "--cluster", cluster_name],
        name == "podman" and args == [
            "tag", "wildside-backend:local", "docker.io/library/wildside-backend:local",
        ],
        is_podman_save,
        is_kind_load,
        is_helm_upgrade,
        name == "kubectl" and contextual(
            "--context", ["get", "namespace", "wildside", "--ignore-not-found"],
        ),
        name == "kubectl" and contextual("--context", ["create", "namespace", "wildside"]),
        is_session_secret_get,
        is_session_secret_create,
    ))
    if name == "k3d" and args == ["cluster", "list", "--output", "json"]:
        print('[{"name":"wildside-preview"}]' if has_cluster() else "[]")
    elif name == "k3d" and args == [
        "cluster", "create", cluster_name, "--servers", "1", "--agents", "1",
        "--port", "127.0.0.1:8088:80@loadbalancer", "--wait",
    ]:
        state_path.write_text("created")
    elif name == "k3d" and args == ["cluster", "delete", cluster_name]:
        state_path.unlink(missing_ok=True)
    elif name == "kind" and args == ["get", "clusters"]:
        print("wildside-preview" if has_cluster() else "other")
    elif name == "kind" and args == [
        "create", "cluster", "--name", cluster_name, "--config", "-", "--wait", "180s",
    ] and stdin_text:
        state_path.write_text("created")
    elif name == "kind" and args == ["delete", "cluster", "--name", cluster_name]:
        state_path.unlink(missing_ok=True)
    elif is_no_output_command:
        pass
    elif name == "helm" and args in (
        ["--kube-context", contexts[0], "-n", "wildside", "status", "wildside"],
        ["--kube-context", contexts[1], "-n", "wildside", "status", "wildside"],
    ):
        print("helm status")
    elif name == "kubectl" and contextual(
        "--context", ["-n", "wildside", "logs", "-l",
        "app.kubernetes.io/instance=wildside", "-c", "app", "--tail", "200"],
    ):
        print("backend log")
    elif name == "kubectl" and contextual(
        "--context", ["-n", "wildside", "get", "pods", "-l",
        "app.kubernetes.io/instance=wildside", "-o", "wide"],
    ):
        print("pod/wildside-backend Running")
    elif name == "kubectl" and contextual(
        "--context", ["-n", "wildside", "get", "service", "wildside", "--ignore-not-found"],
    ):
        print("service/wildside")
    else:
        print(f"unexpected fake command: {name} {args!r}", file=sys.stderr)
        raise SystemExit(1)
    """
)

FAKE_TOOL_NAMES = "docker podman helm k3d kind kubectl uv env systemd-run".split()


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
        assert isinstance(decoded, list), (
            f"fake tool log line {line_number} must decode to a list, "
            f"got {type(decoded).__name__}"
        )
        entries.append(decoded)
    return entries


@pytest.mark.parametrize(
    ("source", "expected", "expected_error", "message"),
    [
        pytest.param("[]\n", [[]], None, None, id="empty-list"),
        pytest.param(
            '["docker", ["build"], false]\n[1, 2]\n',
            [["docker", ["build"], False], [1, 2]],
            None,
            None,
            id="multiple-lists",
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
        assert _load_log_entries(log_path) == expected
    else:
        with pytest.raises(expected_error, match=message):
            _load_log_entries(log_path)


def _assert_command_logged(
    log_entries: list[list[object]],
    tool: str,
    predicate: Callable[[list[object]], bool],
    message: str,
) -> None:
    """Assert a fake-tool log contains a matching command."""
    assert any(
        entry[0] == tool and predicate(cast("list[object]", entry[1]))
        for entry in log_entries
    ), f"{message}; recorded commands: {log_entries!r}"


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
    assert unexpected.returncode != 0
    assert f"unexpected fake command: {container_engine}" in unexpected.stderr
