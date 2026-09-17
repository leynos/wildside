"""Contract tests for the single pull-request execution of the backend suite.

Until 2026-09-17 a pull request ran the backend tests twice: `ci.yml`'s
`build` job at `--all-features` on the backend manifest, and its
`coverage` job through the shared coverage action at three named
features. The two sets were measured with `cargo nextest list
--list-type full` under each invocation on 2026-09-16: the build job's
1643 tests were a strict subset of the coverage job's 1788, with nothing
in the build job alone. The 145 extra were the `example-data`,
`pagination` and `architecture-lint` workspace members, which the
coverage action reaches because it appends `--workspace` to the manifest
it detects and the build job did not.

The build job's step was therefore deleted, and these contracts hold the
two facts that made the deletion safe rather than merely convenient:

1. The surviving execution is reachable. It runs in a coverage lane, on
   `pull_request`, under no condition that can skip it, with the
   features whose selection was the superset, through nextest, and under
   the same nextest profile the deleted step ran under. Break any one of
   those and the repository stops running the backend suite on a pull
   request while every other gate stays green.
2. The compile-fail suites still run. Neither lane ever executed them:
   the deleted step excluded both binaries by name, and the coverage
   run never enables `trybuild-tests`. They ran, and still run, in the
   build job's own `Compile-fail tests` step under plain `cargo test`.

The profile is asserted by absence rather than by value on purpose.
Neither invocation passed `--profile` and neither declared
`NEXTEST_PROFILE` at any scope, so both ran `profile.default` with its
60 s slow timeout and its `pg-embed` serialization group. Asserting that
no scope declares the variable is what makes a later `NEXTEST_PROFILE`
on the coverage job fail here rather than silently change the surviving
lane into a different run from the one that was measured.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

import nextest_profile as profiles
import repository_reading as reading
from timeout_budgets import COVERAGE_ACTION

WORKFLOW_LABEL = "ci.yml"
WORKFLOW_PATH = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / WORKFLOW_LABEL
)

#: The coverage job runs on pull requests and not for Dependabot. Pinned
#: verbatim rather than merely required to be absent, so narrowing it to
#: a push-only event or to `false` fails here.
COVERAGE_JOB_CONDITION = (
    "github.actor != 'dependabot[bot]' && github.event_name != 'push'"
)

#: The feature set whose test list was measured as the superset. Written
#: as the single string the action receives, because that is what the
#: workflow declares and what a reviewer has to compare against.
MEASURED_FEATURES = "example-data metrics test-support"

#: The build job's compile-fail step, whole. Both commands are asserted
#: as the step's entire script: matching one line of a multiline script
#: is satisfied by `if false; then <command>; fi`, and matching a
#: substring is satisfied by `<command> || true`.
COMPILE_FAIL_SCRIPT = (
    "cargo test --locked --all-features -p backend --test "
    "declare_test_support_compile_fail\n"
    "cargo test --locked --features trybuild-tests -p pagination "
    "cursor_trait_bound_compile_fail_tests\n"
)

#: Ways a step can execute the test suite. A step running any of these
#: outside the compile-fail step would be a second execution of the work
#: this change removed, which is the thing that must not come back
#: unnoticed.
TEST_INVOCATIONS = ("cargo nextest run", "cargo test")


def _document() -> dict[str, object]:
    """Return the parsed CI workflow.

    Returns
    -------
    dict[str, object]
        The whole document, read at the one named filesystem boundary
        the contract suite owns.
    """
    parsed = reading.parse_workflow(reading.read_text(WORKFLOW_PATH), WORKFLOW_PATH)
    assert parsed is not None, f"{WORKFLOW_LABEL} must declare a mapping at its top"
    return dict(parsed)


def _triggers(document: dict[str, object]) -> dict[str, object]:
    """Return the workflow's ``on`` mapping.

    YAML 1.1 parses a bare ``on`` key as the boolean ``True``, so the
    document is keyed on either spelling depending on how it was
    written.

    Parameters
    ----------
    document : dict[str, object]
        The parsed workflow.

    Returns
    -------
    dict[str, object]
        The triggers mapping.
    """
    raw = document.get(True, document.get("on"))
    assert isinstance(raw, dict), f"{WORKFLOW_LABEL} must declare triggers as a mapping"
    return typ.cast("dict[str, object]", raw)


def _job(document: dict[str, object], job_name: str) -> dict[str, object]:
    """Return one job's mapping.

    Parameters
    ----------
    document : dict[str, object]
        The parsed workflow.
    job_name : str
        The job's identifier.

    Returns
    -------
    dict[str, object]
        The job.
    """
    jobs = document.get("jobs")
    assert isinstance(jobs, dict), f"{WORKFLOW_LABEL} must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"{WORKFLOW_LABEL} must declare the {job_name} job"
    return typ.cast("dict[str, object]", job)


def _steps(job: dict[str, object], job_name: str) -> list[dict[str, object]]:
    """Return one job's steps, each checked to be a mapping.

    Parameters
    ----------
    job : dict[str, object]
        The parsed job.
    job_name : str
        The job's identifier, for the message.

    Returns
    -------
    list[dict[str, object]]
        The steps, in the order the job runs them.
    """
    steps = job.get("steps")
    assert isinstance(steps, list), f"the {job_name} job must declare steps"
    for index, step in enumerate(steps):
        assert isinstance(step, dict), f"{job_name} step {index} must be a mapping"
    return typ.cast("list[dict[str, object]]", steps)


def _coverage_steps(job: dict[str, object]) -> list[dict[str, object]]:
    """Return the steps in one job that invoke the shared coverage action.

    Parameters
    ----------
    job : dict[str, object]
        The parsed job.

    Returns
    -------
    list[dict[str, object]]
        The matching steps.
    """
    return [
        step
        for step in _steps(job, "coverage")
        if COVERAGE_ACTION in str(step.get("uses", ""))
    ]


def _script(step: dict[str, object]) -> str | None:
    """Return a step's whole ``run`` script when it has one.

    Parameters
    ----------
    step : dict[str, object]
        The parsed step.

    Returns
    -------
    str or None
        The script, or None when the step runs an action instead.
    """
    run = step.get("run")
    return run if isinstance(run, str) else None


def test_the_backend_suite_runs_in_a_coverage_lane_on_pull_request() -> None:
    """One coverage step carries the whole backend suite on a pull request.

    Scenario: the build job no longer runs the suite, so a pull request's
    only execution of it is the coverage lane. Invariant: that lane
    exists exactly once, the workflow triggers on the event it gates,
    the job's condition is the reviewed one, and the step carries no
    condition of its own. A condition anywhere in that chain leaves the
    backend untested on a pull request with nothing red to show it.
    """
    document = _document()
    assert "pull_request" in _triggers(document), (
        f"{WORKFLOW_LABEL} must trigger on pull_request, or the backend suite "
        "runs on no pull request at all"
    )

    job = _job(document, "coverage")
    assert job.get("if") == COVERAGE_JOB_CONDITION, (
        f"the coverage job's condition must be {COVERAGE_JOB_CONDITION!r}; a "
        "changed condition can skip the only lane that runs the backend suite"
    )
    assert "continue-on-error" not in job, (
        "the coverage job must not continue on error; a job that swallows its "
        "own failure reports success whatever its tests found"
    )

    steps = _coverage_steps(job)
    assert len(steps) == 1, (
        f"expected exactly one coverage step in the coverage job, found {len(steps)}"
    )
    assert "if" not in steps[0], (
        "the coverage step must carry no condition at all; one would skip the "
        "backend suite while leaving every other assertion here true"
    )
    assert "continue-on-error" not in steps[0], (
        "the coverage step must not continue on error; running the suite and "
        "discarding its verdict is the same as not running it"
    )


def test_the_coverage_lane_selects_the_measured_superset() -> None:
    """The surviving lane runs the selection the deleted one was a subset of.

    Scenario: the deletion was justified by a measured test list, and
    that list depends on two inputs. Invariant: the coverage step keeps
    nextest as its runner and keeps the three features whose selection
    was measured. Dropping a feature shrinks the selection below the set
    the deleted step used to cover, and dropping nextest changes both
    which binaries run and how the embedded-Postgres suites are
    serialized.
    """
    options = _coverage_steps(_job(_document(), "coverage"))[0].get("with")
    assert isinstance(options, dict), "the coverage step must declare inputs"
    assert options.get("use-cargo-nextest") == "true", (
        "the coverage step must run through nextest; .config/nextest.toml's "
        "pg-embed test group is what keeps the embedded-Postgres binaries off "
        "one another's cluster directory"
    )
    assert options.get("features") == MEASURED_FEATURES, (
        f"the coverage step must select {MEASURED_FEATURES!r}, the feature set "
        "whose 1788-test list was measured as a superset of the deleted step's "
        "1643"
    )


def test_no_scope_declares_a_nextest_profile_for_the_coverage_lane() -> None:
    """The surviving lane runs profile.default, as the deleted step did.

    Scenario: both invocations ran without ``--profile`` and without
    ``NEXTEST_PROFILE`` at any scope, so both took ``profile.default``
    with its 60 s slow timeout and its ``pg-embed`` serialization group.
    Invariant: no scope declares the variable. A declaration at step,
    job or workflow level would run the surviving lane under a different
    profile from the one the subset was measured under, which is exactly
    the clause that makes a deletion unsafe elsewhere in the estate.
    """
    document = _document()
    job = _job(document, "coverage")
    declaration = profiles.declared_profile(document, job, _coverage_steps(job)[0])
    assert declaration.is_absent(), (
        f"the {declaration.scope} scope declares "
        f"{profiles.PROFILE_VARIABLE}={declaration.value!r}; the coverage lane "
        "must run profile.default, which is the profile the deleted build-job "
        "step ran under and the profile the test-list comparison was measured "
        "under"
    )


def test_the_build_job_runs_the_compile_fail_suites_and_nothing_else() -> None:
    """Compile-fail suites survive the deletion, and no test lane returns.

    Scenario: the compile-fail binaries were excluded from both
    invocations by name, so they are the one thing the deleted step's
    neighbourhood still owes. Invariant: the build job runs both
    commands as one step's entire script, and runs no other cargo test
    invocation. The second half is what keeps the duplicate lane from
    coming back under another step name.
    """
    steps = _steps(_job(_document(), "build"), "build")
    compile_fail = [step for step in steps if _script(step) == COMPILE_FAIL_SCRIPT]
    assert len(compile_fail) == 1, (
        "expected exactly one build step whose whole script is the two "
        f"compile-fail cargo test commands, found {len(compile_fail)}"
    )
    assert "if" not in compile_fail[0], (
        "the compile-fail step must carry no condition; nothing else in the "
        "repository runs those two suites"
    )

    others = [
        step.get("name")
        for step in steps
        if step is not compile_fail[0]
        and any(command in (_script(step) or "") for command in TEST_INVOCATIONS)
    ]
    assert others == [], (
        f"these build steps run the test suite again: {others}; a pull "
        "request's backend suite belongs in the coverage lane alone"
    )
