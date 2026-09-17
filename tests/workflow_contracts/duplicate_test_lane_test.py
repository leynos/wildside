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

The build job's step was therefore deleted, and with it the four steps
whose only consumer it was: the nextest install, the pg_worker install,
the embedded PostgreSQL warm-up and that database cache's restore. These
contracts hold the three facts that made the deletion safe rather than
merely convenient:

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
3. The build job acquires no test tooling and no database binaries. The
   four steps that served the deleted suite are gone, and none of them
   can come back by imitation: a job that installs a test runner or
   restores a PostgreSQL archive is a job preparing to run the suite
   again, whatever the step is called.

The profile is asserted by absence rather than by value on purpose.
Neither invocation passed `--profile` and neither declared
`NEXTEST_PROFILE` at any scope, so both ran `profile.default` with its
60 s slow timeout and its `pg-embed` serialization group. Asserting that
no scope declares the variable is what makes a later `NEXTEST_PROFILE`
on the coverage job fail here rather than silently change the surviving
lane into a different run from the one that was measured.
"""

from __future__ import annotations

import ci_lane_reading as lanes
import nextest_profile as profiles

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

#: Commands that acquire test tooling, each named as the command rather
#: than as the tool. A step name is prose and an installed tool leaves no
#: trace in the workflow; the command is the thing that has to be absent.
TOOLING_COMMANDS = (
    "make prepare-pg-worker",
    "scripts/warm-pg-embedded-cache.sh",
    "pg-embed-setup-unpriv",
)

#: The installer action, and the tool prefixes the build job must not ask
#: it for. `taiki-e/install-action` names its tool in an input rather than
#: in a script, so the script search above cannot see it.
INSTALL_ACTION = "taiki-e/install-action"
INSTALL_ACTION_DENIED = ("nextest",)

#: Cache paths holding embedded PostgreSQL binaries. A job restoring one
#: is a job preparing to start a database, which the build job no longer
#: does. Both are real path entries; the `#`-prefixed lines in the same
#: block scalar are the cache action's comments, not paths.
DATABASE_CACHE_PATHS = ("~/.theseus/postgresql", "~/.cache/pg-embedded/binaries")

#: Ways a step can execute the test suite. A step running any of these
#: outside the compile-fail step would be a second execution of the work
#: this change removed, which is the thing that must not come back
#: unnoticed.
TEST_INVOCATIONS = ("cargo nextest run", "cargo test")


def test_the_backend_suite_runs_in_a_coverage_lane_on_pull_request() -> None:
    """One coverage step carries the whole backend suite on a pull request.

    Scenario: the build job no longer runs the suite, so a pull request's
    only execution of it is the coverage lane. Invariant: that lane
    exists exactly once, the workflow triggers on the event it gates,
    the job's condition is the reviewed one, and the step carries no
    condition of its own. A condition anywhere in that chain leaves the
    backend untested on a pull request with nothing red to show it.
    """
    document = lanes.ci_workflow()
    assert "pull_request" in lanes.triggers_of(document), (
        f"{lanes.WORKFLOW_LABEL} must trigger on pull_request, or the backend suite "
        "runs on no pull request at all"
    )

    job = lanes.job_named(document, "coverage")
    assert job.get("if") == COVERAGE_JOB_CONDITION, (
        f"the coverage job's condition must be {COVERAGE_JOB_CONDITION!r}; a "
        "changed condition can skip the only lane that runs the backend suite"
    )
    assert "continue-on-error" not in job, (
        "the coverage job must not continue on error; a job that swallows its "
        "own failure reports success whatever its tests found"
    )

    steps = lanes.coverage_steps_of(job)
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
    options = lanes.coverage_steps_of(lanes.job_named(lanes.ci_workflow(), "coverage"))[
        0
    ].get("with")
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
    document = lanes.ci_workflow()
    job = lanes.job_named(document, "coverage")
    declaration = profiles.declared_profile(
        document, job, lanes.coverage_steps_of(job)[0]
    )
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
    steps = lanes.steps_of(lanes.job_named(lanes.ci_workflow(), "build"), "build")
    compile_fail = [
        step for step in steps if lanes.script_of(step) == COMPILE_FAIL_SCRIPT
    ]
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
        and any(
            command in (lanes.script_of(step) or "") for command in TEST_INVOCATIONS
        )
    ]
    assert others == [], (
        f"these build steps run the test suite again: {others}; a pull "
        "request's backend suite belongs in the coverage lane alone"
    )


def _build_steps() -> list[dict[str, object]]:
    """Return the build job's steps.

    Returns
    -------
    list[dict[str, object]]
        The steps, in the order the job runs them.
    """
    return lanes.steps_of(lanes.job_named(lanes.ci_workflow(), "build"), "build")


def test_the_build_job_runs_no_tooling_acquisition_command() -> None:
    """No build step fetches the worker binary or warms the database.

    Scenario: the deleted suite was the only consumer of the pg_worker
    install and the PostgreSQL warm-up, so both went with it. Invariant:
    no step runs either command. The commands are named rather than the
    steps, because a step name is prose and renaming one must not
    satisfy this.
    """
    acquisitions = [
        (step.get("name"), command)
        for step in _build_steps()
        for command in TOOLING_COMMANDS
        if command in (lanes.script_of(step) or "")
    ]
    assert acquisitions == [], (
        f"these build steps acquire test tooling: {acquisitions}; the build "
        "job runs no test suite, so a database worker or a warmed PostgreSQL "
        "archive in it is either dead weight or a duplicate lane returning"
    )


def test_the_build_job_installs_no_test_runner() -> None:
    """No build step asks the installer action for a test runner.

    Scenario: `taiki-e/install-action` names its tool in an input rather
    than in a script, so the command search above cannot see it.
    Invariant: no step in the build job asks it for nextest. The
    coverage lane's runner is chosen by the pinned shared action, and
    nothing else in the repository runs nextest in CI.
    """
    installed = [
        (step.get("name"), tool)
        for step in _build_steps()
        if INSTALL_ACTION in str(step.get("uses", ""))
        for tool in INSTALL_ACTION_DENIED
        if isinstance(options := step.get("with"), dict)
        and str(options.get("tool", "")).startswith(tool)
    ]
    assert installed == [], (
        f"these build steps install a test runner: {installed}; only the "
        "coverage lane runs the suite, and the shared action chooses its own "
        "nextest"
    )


def test_the_build_job_restores_no_database_binaries() -> None:
    """No build cache step carries an embedded PostgreSQL archive.

    Scenario: the database cache's restore served the deleted suite
    alone. Invariant: no cache step in the build job lists either
    archive path, including by appending one to a cache that has an
    honest purpose. Only a job that starts a database needs them.
    """
    restores = [
        (step.get("name"), path)
        for step in _build_steps()
        for path in lanes.cache_paths_of(step)
        if path in DATABASE_CACHE_PATHS
    ]
    assert restores == [], (
        f"these build cache steps carry embedded PostgreSQL binaries: "
        f"{restores}; only a job that starts a database needs them"
    )
