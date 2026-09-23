"""Contracts closing the routes around the CodeScene absence contracts.

The absence contracts in `codescene_coverage_baseline_test.py` are only
as wide as the set of workflows they are parametrized over, and only as
strict as what they look for. This module holds three routes each of
them would have missed:

- A reusable workflow a pull-request workflow calls runs on that pull
  request, with `secrets: inherit` handing it the credential, while
  declaring no pull-request trigger itself. The set is a closure.
- A step can curl the CodeScene API, naming neither the action, the CLI
  nor the credential. The service's host is refused in its own right.
- `secrets: inherit` into another repository's workflow hands the
  credential to code this tree does not hold, so no contract can see
  what it does with it.

It also holds the two readers everything above rests on: the trigger
reader, in each form GitHub accepts, and the strict loader, which
refuses a mapping that repeats a key rather than keeping the last one.

The repository's own workflows cannot show most of this, because none
calls another today. The cases therefore build small workflow trees and
point the inventory at them, so each route is shown both passing the old
reading and failing this one.
"""

from __future__ import annotations

import typing as typ

import pytest
import strict_yaml
import workflow_inventory as inventory
import yaml
from codescene_baseline import CODESCENE_HOST
from codescene_coverage_baseline_test import (
    test_no_pull_request_workflow_holds_the_codescene_credential as holds_credential,
)
from workflow_calls import (
    UnresolvedWorkflowCallError,
    inherits_into_other_repositories,
    local_workflow_name,
)

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    from pathlib import Path

#: The workflow episodic measured passing every clause of its contract.
PROBE = """\
"on":
  workflow_call:
jobs:
  probe:
    runs-on: ubuntu-latest
    steps:
      - name: Probe the CodeScene project
        env:
          CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}
        run: curl -fsS https://api.codescene.io/v2/projects/1 > /dev/null
"""


def _caller(reference: str, trigger: str = "pull_request") -> str:
    """Return a workflow on ``trigger`` calling ``reference`` with every secret."""
    return (
        f'"on": {trigger}\n'
        "jobs:\n"
        "  call:\n"
        f"    uses: {reference}\n"
        "    secrets: inherit\n"
    )


def _tree(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, **files: str) -> None:
    """Write workflows named with ``_`` for ``.`` and point the inventory there."""
    for key, text in files.items():
        (tmp_path / key.replace("_", ".")).write_text(text, encoding="utf-8")
    # `raising` stays at its default, so a renamed constant fails loudly
    # rather than letting the case pass having patched nothing.
    monkeypatch.setattr(inventory, "WORKFLOWS_DIR", tmp_path)


def _contacts_the_host(filename: str) -> bool:
    """Return whether a workflow's text names the CodeScene host, in any case."""
    text = (inventory.WORKFLOWS_DIR / filename).read_text(encoding="utf-8")
    return CODESCENE_HOST in text.casefold()


@pytest.mark.parametrize("filename", inventory.pull_request_workflows())
def test_no_pull_request_workflow_contacts_the_codescene_host(filename: str) -> None:
    """No pull-request workflow names the CodeScene host.

    Scenario: every workflow a pull request runs, read as raw text so a
    URL in a script, an input or a comment is found alike. Invariant: the
    host appears nowhere, in any case.
    """
    assert not _contacts_the_host(filename), (
        f"{filename} runs on a pull request and names {CODESCENE_HOST}; a step "
        "can reach the service by curling it without the action or the CLI"
    )


@pytest.mark.parametrize("filename", inventory.pull_request_workflows())
def test_no_pull_request_workflow_inherits_into_another_repository(
    filename: str,
) -> None:
    """No pull-request workflow hands every secret to code outside this tree.

    Scenario: every workflow a pull request runs. Invariant: no job
    calling another repository's reusable workflow passes
    `secrets: inherit`. A local call may, because its callee is in the
    closure and held to every contract here.
    """
    offenders = inherits_into_other_repositories(inventory.load_workflow(filename))
    assert offenders == [], (
        f"{filename} runs on a pull request and passes every secret, the "
        f"CodeScene credential included, to another repository in {offenders}"
    )


@pytest.mark.parametrize(
    "reference",
    [
        "./.github/workflows/probe.yml",
        "$/.github/workflows/probe.yml",
    ],
)
def test_the_lane_follows_calls_to_the_probe(
    reference: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A called `workflow_call` workflow is in the lane, in either spelling.

    The trigger reading alone finds the caller only, and the credential
    contract then never reads the probe that holds the token.
    """
    _tree(tmp_path, monkeypatch, ci_yml=_caller(reference), probe_yml=PROBE)
    assert inventory.pull_request_workflows() == ["ci.yml", "probe.yml"], (
        f"a workflow called as {reference!r} runs on the pull request"
    )
    with pytest.raises(AssertionError, match="CS_ACCESS_TOKEN"):
        holds_credential("probe.yml")
    assert _contacts_the_host("probe.yml"), "the probe's curl must be found"


def test_the_host_is_found_in_any_case(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A mixed-case host is the same host, because DNS names ignore case."""
    _tree(
        tmp_path, monkeypatch, probe_yml=PROBE.replace("codescene.io", "CodeScene.IO")
    )
    assert _contacts_the_host("probe.yml"), "the host must be matched case-blind"


def test_the_lane_is_transitive_and_stays_narrow(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A callee's own call is followed; an uncalled reusable workflow is not.

    The second half keeps a complying repository from failing for merely
    holding a reusable workflow a push lane uses.
    """
    middle = _caller("./.github/workflows/probe.yml", trigger="workflow_call")
    _tree(
        tmp_path,
        monkeypatch,
        ci_yml=_caller("./.github/workflows/middle.yml"),
        middle_yml=middle,
        probe_yml=PROBE,
        uncalled_yml=PROBE,
    )
    lane = inventory.pull_request_workflows()
    assert lane == ["ci.yml", "middle.yml", "probe.yml"], (
        f"the lane must be exactly the transitive closure, got {lane}"
    )


def test_an_unresolved_local_call_fails_the_reading(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A call the closure cannot read fails rather than dropping out."""
    _tree(tmp_path, monkeypatch, ci_yml=_caller("./.github/workflows/gone.yml"))
    with pytest.raises(UnresolvedWorkflowCallError, match=r"gone\.yml"):
        inventory.pull_request_workflows()


@pytest.mark.parametrize(
    ("reference", "expected"),
    [
        ("./.github/workflows/release.yml", "release.yml"),
        ("$/.github/workflows/release.yml", "release.yml"),
        ("$/.github/workflows/nested/release.yml", None),
        ("./.github/workflows/nested/release.yml", None),
        ("./.github/actions/setup", None),
        ("leynos/wildside/.github/workflows/release.yml@main", None),
        ("./.github/workflows/", None),
    ],
)
def test_a_local_call_is_read_by_shape(reference: str, expected: str | None) -> None:
    """A call is local exactly when it names a file under the directory."""
    assert local_workflow_name(reference) == expected, (
        f"{reference!r} must read as {expected!r}"
    )


def test_inheriting_into_another_repository_is_reported() -> None:
    """Only the cross-repository inherit is an offender."""
    document = strict_yaml.load(
        _caller("leynos/shared-actions/.github/workflows/x.yml@abc123")
    )
    assert isinstance(document, dict), "the caller must parse to a mapping"
    assert inherits_into_other_repositories(document) == ["call"], (
        "a cross-repository inherit must be reported"
    )
    local = strict_yaml.load(_caller("./.github/workflows/x.yml"))
    assert isinstance(local, dict), "the caller must parse to a mapping"
    assert inherits_into_other_repositories(local) == [], (
        "a local inherit is read through the closure, not refused"
    )


@pytest.mark.parametrize(
    "source",
    [
        "on: pull_request\n",
        '"on": pull_request\n',
        "on: [push, pull_request]\n",
        "on:\n  pull_request:\n    branches: [main]\n",
    ],
)
def test_every_trigger_form_is_read(source: str) -> None:
    """A name, a list and a mapping are each read, under either `on` key.

    Each is parsed from real text, so the bare key arrives as the boolean
    YAML 1.1 makes of it. A mapping-only reader would read the list form
    as nothing and the workflow would leave the pull-request set.
    """
    document = strict_yaml.load(source + "jobs: {}\n")
    assert isinstance(document, dict), "the workflow must parse to a mapping"
    assert "pull_request" in inventory.triggers_of(document), (
        f"{source.strip()!r} must read as a pull-request trigger"
    )


def test_a_repeated_key_is_refused(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A job declaring `runs-on` twice fails to load, not to the last value.

    PyYAML would keep the second label silently, so a paid label in the
    first half would read as hosted to every placement contract.
    """
    doubled = (
        '"on": pull_request\n'
        "jobs:\n"
        "  build:\n"
        "    runs-on: ubicloud-standard-8\n"
        "    runs-on: ubuntu-latest\n"
    )
    _tree(tmp_path, monkeypatch, ci_yml=doubled)
    with pytest.raises(yaml.YAMLError, match="runs-on"):
        inventory.load_workflow("ci.yml")


@pytest.mark.parametrize(
    "source",
    [
        pytest.param('on: push\n"on": pull_request\n', id="bare-then-quoted"),
        pytest.param("'on': push\non: pull_request\n", id="quoted-then-bare"),
    ],
)
def test_both_spellings_of_on_are_refused(source: str) -> None:
    """A workflow declaring a bare `on` and a quoted `"on"` fails to load.

    PyYAML constructs them as two keys, `True` and `"on"`; GitHub reads both
    as `on` and merges them. A trigger reader that saw one half would be
    blind to the other, so the pull-request set could drop a workflow whose
    `pull_request` trigger sat in the unread half.
    """
    with pytest.raises(yaml.YAMLError, match="'on'"):
        strict_yaml.load(source + "jobs: {}\n")


def test_one_spelling_of_on_beside_other_keys_loads() -> None:
    """Keys written differently are not a repeat, whatever they construct to."""
    document = strict_yaml.load('"on": push\njobs: {}\nname: ci\n')
    assert document == {"on": "push", "jobs": {}, "name": "ci"}, (
        f"distinct keys must survive the strict loader, got {document!r}"
    )


def test_an_unprefixed_local_call_is_refused() -> None:
    """A workflow-directory reference with neither documented prefix fails.

    GitHub documents `./` and `$/` for a same-repository call. Reading a
    bare `.github/workflows/` reference as another repository's call would
    drop its callee from the lane in silence.
    """
    with pytest.raises(UnresolvedWorkflowCallError, match="without"):
        local_workflow_name(".github/workflows/release.yml")
