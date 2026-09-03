"""Shared readers for the repository's workflow-contract suites.

The runner-platform contracts assert properties of the whole workflow estate
rather than of one file, so the loading, job iteration, and cache-step
vocabulary they share live here instead of being repeated per suite.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import yaml

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS_DIR = REPOSITORY_ROOT / ".github" / "workflows"
ACTIONLINT_CONFIG = REPOSITORY_ROOT / ".github" / "actionlint.yaml"
MAKEFILE_PATH = REPOSITORY_ROOT / "Makefile"

#: The cache actions. One reviewed upstream version serves both the managed
#: and GitHub-hosted lanes, because the transparent cache proxy in front of
#: the managed runners intercepts its traffic.
CACHE_ACTION_PATHS = ("actions/cache", "actions/cache/restore", "actions/cache/save")

#: Managed-runner labels this repository intends to use. Every label here must
#: also be registered with actionlint.
MANAGED_RUNNER_LABELS = frozenset({"ubicloud-standard-8"})
GITHUB_HOSTED_LABELS = frozenset({"ubuntu-latest"})

#: Jobs that build or test the product. Everything else is administrative,
#: scheduled, or API-bound and belongs on a GitHub-hosted runner.
BUILD_JOBS = frozenset({
    ("ci.yml", "build"),
    ("ci.yml", "coverage"),
    ("coverage-main.yml", "coverage-upload"),
})

_CACHE_KEY_ASSIGNMENT = re.compile(
    r"printf '(?P<name>[A-Z0-9_]+)=(?P<prefix>[a-z0-9-]+?)-%s", re.MULTILINE
)
_ENV_REFERENCE = re.compile(r"^\$\{\{\s*env\.(?P<name>[A-Z0-9_]+)\s*\}\}$")


def load_workflow(filename: str) -> dict[str, typ.Any]:
    """Parse one workflow file into a mapping.

    Examples
    --------
    >>> workflow = load_workflow("ci.yml")
    >>> sorted(workflow["jobs"])
    ['build', 'coverage']
    """
    document = yaml.safe_load((WORKFLOWS_DIR / filename).read_text(encoding="utf-8"))
    if not isinstance(document, dict):
        message = f"{filename} must parse to a mapping"
        raise TypeError(message)
    return document


def workflow_filenames() -> list[str]:
    """Return every workflow filename, sorted for stable test identifiers.

    Both suffixes are collected. GitHub accepts either, and the repository's
    own workflow lint already globs both, so a file named `something.yaml`
    would otherwise escape every contract built on this helper.
    """
    return sorted(
        path.name
        for suffix in ("*.yml", "*.yaml")
        for path in WORKFLOWS_DIR.glob(suffix)
    )


def workflow_jobs(filename: str) -> list[tuple[str, dict[str, typ.Any]]]:
    """Return one workflow's ``(job_id, job)`` pairs.

    Examples
    --------
    >>> [job_id for job_id, _ in workflow_jobs("coverage-main.yml")]
    ['coverage-upload']
    """
    jobs = load_workflow(filename).get("jobs", {})
    if not isinstance(jobs, dict):
        message = f"{filename} must declare a jobs mapping"
        raise TypeError(message)
    return [(job_id, job) for job_id, job in jobs.items() if isinstance(job, dict)]


def iter_jobs() -> cabc.Iterator[tuple[str, str, dict[str, typ.Any]]]:
    """Yield ``(filename, job_id, job)`` for every job in the estate."""
    for filename in workflow_filenames():
        for job_id, job in workflow_jobs(filename):
            yield filename, job_id, job


def job_steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return a job's steps, or an empty list for a reusable-workflow caller."""
    steps = job.get("steps", [])
    if not isinstance(steps, list):
        message = "a job's steps must be a sequence"
        raise TypeError(message)
    return [step for step in steps if isinstance(step, dict)]


def step_action(step: dict[str, typ.Any]) -> str:
    """Return the action path a step uses, without its ref."""
    uses = step.get("uses")
    return uses.partition("@")[0] if isinstance(uses, str) else ""


def is_cache_step(step: dict[str, typ.Any]) -> bool:
    """Report whether a step is one of the cache actions."""
    return step_action(step) in CACHE_ACTION_PATHS


def cache_paths(step: dict[str, typ.Any]) -> list[str]:
    """Return the mutable paths a cache step claims ownership of."""
    options = step.get("with")
    if not isinstance(options, dict):
        return []
    raw = options.get("path")
    if not isinstance(raw, str):
        return []
    return [line.strip() for line in raw.splitlines() if line.strip()]


def cache_key_prefixes(job: dict[str, typ.Any]) -> dict[str, str]:
    """Map each cache-key environment variable to its literal key prefix.

    The jobs compose their keys in a shell step so the runner image identity
    can join the expression-visible inputs. Reading the prefixes back out lets
    a contract compare writers across workflows by the key they publish rather
    than by the variable name that happens to hold it.

    Examples
    --------
    >>> job = load_workflow("coverage-main.yml")["jobs"]["coverage-upload"]
    >>> cache_key_prefixes(job)["CARGO_CACHE_KEY"]
    'cargo-v1'
    """
    prefixes: dict[str, str] = {}
    for step in job_steps(job):
        script = step.get("run")
        if not isinstance(script, str):
            continue
        for match in _CACHE_KEY_ASSIGNMENT.finditer(script):
            prefixes[match.group("name")] = match.group("prefix")
    return prefixes


def resolved_cache_key(step: dict[str, typ.Any], prefixes: dict[str, str]) -> str:
    """Return the key prefix a cache step reserves, or its raw key expression.

    A key that names an environment variable with no parsed prefix is an
    error rather than a fallback. Returning the variable name would let the
    ownership and writer contracts compare names instead of published keys
    and keep passing after a reformatted key assignment.
    """
    options = step.get("with")
    key = options.get("key") if isinstance(options, dict) else None
    if not isinstance(key, str):
        return ""
    match = _ENV_REFERENCE.match(key.strip())
    if match is None:
        return key
    name = match.group("name")
    if name not in prefixes:
        message = f"no cache-key prefix was parsed for {name}"
        raise AssertionError(message)
    return prefixes[name]


def find_step(steps: list[dict[str, typ.Any]], name: str) -> dict[str, typ.Any]:
    """Return the uniquely named step, raising when it is absent or repeated."""
    matches = [step for step in steps if step.get("name") == name]
    if len(matches) != 1:
        message = f"expected one {name!r} step, found {len(matches)}"
        raise AssertionError(message)
    return matches[0]


def step_index(steps: list[dict[str, typ.Any]], name: str) -> int:
    """Return the position of the uniquely named step."""
    return steps.index(find_step(steps, name))


def shell_commands(step: dict[str, typ.Any]) -> list[str]:
    r"""Return a step's script as logical commands, with comments removed.

    Continuation lines are joined so a flag on the second line of an
    invocation is still part of the command a contract inspects, and comment
    lines are dropped so prose about a banned form is not mistaken for the
    form itself.

    Examples
    --------
    >>> shell_commands({"run": "# note\\ncargo build \\\\\\n  --locked\\n"})
    ['cargo build --locked']
    """
    script = step.get("run")
    if not isinstance(script, str):
        return []
    commands: list[str] = []
    pending = ""
    for raw_line in script.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line.endswith("\\"):
            pending += line[:-1].rstrip() + " "
            continue
        commands.append((pending + line).strip())
        pending = ""
    if pending:
        commands.append(pending.strip())
    return commands


def iter_step_commands() -> cabc.Iterator[tuple[str, str, str, str]]:
    """Yield ``(filename, job_id, step_name, command)`` for every shell command."""
    for filename, job_id, job in iter_jobs():
        for step in job_steps(job):
            name = str(step.get("name", step.get("uses", "")))
            for command in shell_commands(step):
                yield filename, job_id, name, command
