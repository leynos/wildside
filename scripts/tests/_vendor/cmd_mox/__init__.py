"""Minimal cmd-mox compatible helpers for command mocking in tests."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Iterable, List, Tuple

from plumbum import ProcessExecutionError


@dataclass
class CommandCall:
    args: Tuple[str, ...]
    env: Dict[str, str]


@dataclass
class CommandResponse:
    args: Tuple[str, ...]
    stdout: str = ""
    stderr: str = ""
    exit_code: int = 0


class MockCommandInvocation:
    def __init__(self, command: "MockCommand", args: Iterable[str], env: Dict[str, str] | None = None):
        self._command = command
        self._args = tuple(args)
        self._env = dict(env or {})

    def __getitem__(self, more_args: Iterable[str] | str) -> "MockCommandInvocation":
        if isinstance(more_args, tuple):
            args = self._args + more_args
        elif isinstance(more_args, list):
            args = self._args + tuple(more_args)
        else:
            args = self._args + (more_args,)
        return MockCommandInvocation(self._command, args, self._env)

    def with_env(self, **env: str) -> "MockCommandInvocation":
        merged = dict(self._env)
        merged.update(env)
        return MockCommandInvocation(self._command, self._args, merged)

    def __call__(self) -> str:
        return self._command.execute(self._args, self._env)


class MockCommand:
    def __init__(self, name: str):
        self.name = name
        self.calls: List[CommandCall] = []
        self._queue: List[CommandResponse] = []

    def queue(self, *args: str, stdout: str = "", stderr: str = "", exit_code: int = 0) -> None:
        self._queue.append(CommandResponse(tuple(args), stdout, stderr, exit_code))

    def execute(self, args: Tuple[str, ...], env: Dict[str, str]) -> str:
        if not self._queue:
            raise AssertionError(f"No queued responses remaining for command '{self.name}'.")
        response = self._queue.pop(0)
        if response.args and response.args != args:
            raise AssertionError(
                f"Command '{self.name}' expected args {response.args!r} but received {args!r}."
            )
        self.calls.append(CommandCall(args=args, env=env))
        if response.exit_code:
            raise ProcessExecutionError(
                [self.name, *args],
                response.exit_code,
                response.stdout,
                response.stderr,
            )
        return response.stdout

    def __getitem__(self, args: Iterable[str] | str) -> MockCommandInvocation:
        if isinstance(args, tuple):
            values = args
        elif isinstance(args, list):
            values = tuple(args)
        else:
            values = (args,)
        return MockCommandInvocation(self, values)

    def with_env(self, **env: str) -> MockCommandInvocation:
        return MockCommandInvocation(self, tuple(), env)


class LocalProxy:
    def __init__(self, registry: "CommandRegistry"):
        self._registry = registry

    def __getitem__(self, name: str) -> MockCommand:
        if name not in self._registry.commands:
            raise KeyError(f"Command '{name}' not registered in CommandRegistry")
        return self._registry.commands[name]


class CommandRegistry:
    def __init__(self):
        self.commands: Dict[str, MockCommand] = {}
        self.local_proxy = LocalProxy(self)

    def create(self, name: str) -> MockCommand:
        command = MockCommand(name)
        self.commands[name] = command
        return command

    def attach(self, module: Any) -> None:
        module.local = self.local_proxy

    def detach(self, module: Any) -> None:
        raise NotImplementedError("Detach is not supported by this stub.")


__all__ = [
    "CommandRegistry",
    "MockCommand",
]
