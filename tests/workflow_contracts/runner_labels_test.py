"""Contract tests for the `runs-on` reader every placement contract uses.

`runner_labels` decides which labels the placement, registration and
fork-fallback contracts see. Before, it returned nothing for the mapping
form of `runs-on` (`group:` and `labels:`) and for an expression naming
no label. Nothing was also the answer for a reusable-workflow caller,
which declares no runner, so a paid label written that way was checked
by none of those contracts. The cases here hold the three forms GitHub
accepts, and hold every other shape to a refusal rather than an empty
reading.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pytest
import runner_shapes


@pytest.mark.parametrize(
    ("runner", "expected"),
    [
        pytest.param("ubuntu-latest", {"ubuntu-latest"}, id="label"),
        pytest.param(
            "${{ github.event.pull_request.head.repo.fork && 'ubuntu-latest'"
            " || 'ubicloud-standard-8' }}",
            {"ubuntu-latest", "ubicloud-standard-8"},
            id="expression",
        ),
        pytest.param(
            "${{ 'ubuntu-latest' }}", {"ubuntu-latest"}, id="literal-expression"
        ),
        pytest.param(
            "${{ a && 'x' || b && 'y' || 'z' }}",
            {"x", "y", "z"},
            id="chained-fallbacks",
        ),
        pytest.param(["self-hosted", "linux"], {"self-hosted", "linux"}, id="list"),
        pytest.param(
            {"labels": "ubicloud-standard-8"}, {"ubicloud-standard-8"}, id="labels-text"
        ),
        pytest.param(
            {"labels": ["ubicloud-standard-8", "x64"]},
            {"ubicloud-standard-8", "x64"},
            id="labels-list",
        ),
        pytest.param({"group": "paid"}, {"group:paid"}, id="group"),
        pytest.param(
            {"group": "paid", "labels": ["ubicloud-standard-8"]},
            {"group:paid", "ubicloud-standard-8"},
            id="group-and-labels",
        ),
    ],
)
def test_every_accepted_form_yields_its_labels(
    runner: object, expected: set[str]
) -> None:
    """Each form GitHub accepts is read to the labels it can select.

    The mapping cases are the ones this reader used to answer with
    nothing. A group is reported under its own prefix, so it matches
    neither the managed nor the hosted set and fails every placement
    contract until one models it deliberately.
    """
    assert runner_shapes.runner_labels({"runs-on": runner}) == expected, (
        f"runs-on {runner!r} must read as {sorted(expected)}"
    )


def test_a_job_without_a_runner_yields_nothing() -> None:
    """A reusable-workflow caller declares no runner, and reads as none."""
    job = {"uses": "owner/repo/.github/workflows/x.yml@v1"}
    assert runner_shapes.runner_labels(job) == frozenset(), "a caller selects no runner"


@pytest.mark.parametrize(
    "runner",
    [
        pytest.param(None, id="null"),
        pytest.param(8, id="number"),
        pytest.param([], id="empty-list"),
        pytest.param(["ubuntu-latest", 8], id="list-with-a-number"),
        pytest.param({}, id="empty-mapping"),
        pytest.param({"labels": "a", "size": "large"}, id="unknown-key"),
        pytest.param({"labels": 8}, id="labels-a-number"),
        pytest.param({"group": ""}, id="empty-group"),
        pytest.param("${{ matrix.os }}", id="expression-naming-no-label"),
        pytest.param("${{ matrix.os || 'ubuntu-latest' }}", id="dynamic-first-operand"),
        pytest.param("${{ x && matrix.os || 'ubuntu-latest' }}", id="dynamic-result"),
        pytest.param(
            "${{ x && 'ubicloud-standard-8' }}", id="condition-can-be-the-result"
        ),
        pytest.param("runner-${{ 'a' }}", id="text-around-the-expression"),
        pytest.param("", id="empty-label"),
        pytest.param("${{ '' }}", id="empty-quoted-label"),
        pytest.param("${{ x && '' || 'ubuntu-latest' }}", id="empty-quoted-result"),
        pytest.param(["ubuntu-latest", " "], id="blank-list-entry"),
        pytest.param(
            "${{ a && (b && 'ubicloud-standard-8') || 'ubuntu-latest' }}",
            id="nested-group-as-result",
        ),
    ],
)
def test_an_unmodelled_shape_is_refused(runner: object) -> None:
    """A shape outside the three forms fails loudly instead of reading as empty.

    An empty reading is what a job with no runner looks like, so every
    placement contract would pass over the job. An expression is refused
    whenever any result it can take is not a quoted label:
    `${{ matrix.os || 'ubuntu-latest' }}` names one label and may select
    another, and reading the literal alone would hide the paid one.
    """
    with pytest.raises(runner_shapes.UnreadableRunnerError):
        runner_shapes.runner_labels({"runs-on": runner})
