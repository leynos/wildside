"""Contract tests for where the backend suite runs on a pull request.

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

The build job's step was therefore narrowed rather than deleted, and the
distinction is the whole point of this module. The coverage job carries
`github.actor != 'dependabot[bot]'`, deliberately, to keep dependency
automerge off the expensive coverage path. Deleting the build job's step
outright would have left a Dependabot pull request running no backend
tests at all, then merging itself on green. A test-list comparison
cannot see that: both lanes list the same tests on the event it measures,
and the hole is in an actor clause rather than in a selection.

So the step and the four that serve it survive under the inverse guard,
`github.actor == 'dependabot[bot]'`, and the contracts here hold four
things:

1. The coverage lane is reachable on `pull_request`, under the reviewed
   job condition and with no condition on its step, running nextest with
   the features whose selection was the measured superset, under the
   nextest profile the comparison was made in.
2. The two lanes' conditions are complementary. This is asserted between
   them rather than by pinning each alone, because two conditions that
   are each individually reasonable can still exclude the same actor from
   both, which is the state this repository nearly shipped.
3. Every build step that runs the suite, acquires test tooling or
   restores a database archive carries the Dependabot guard. Unguarded,
   any of them is the duplicate lane returning for every contributor.
4. The compile-fail suites run for everyone, unconditionally, at both job
   and step scope. Neither lane has ever executed them: the guarded step
   excludes both binaries by name and the coverage run never enables
   `trybuild-tests`.

The profile is asserted by absence rather than by value on purpose.
Neither invocation passed `--profile` and neither declared
`NEXTEST_PROFILE` at any scope, so both ran `profile.default` with its
60 s slow timeout and its `pg-embed` serialization group. Asserting that
no scope declares the variable is what makes a later `NEXTEST_PROFILE`
on the coverage job fail here rather than silently turn the surviving
lane into a different run from the one that was measured.
"""

from __future__ import annotations

import ci_lane_reading as lanes
import ci_step_predicates as does
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

#: The guard on the build job's Dependabot test lane. It is the exact
#: inverse of the actor clause in :data:`COVERAGE_JOB_CONDITION`, and the
#: two are asserted against each other rather than each on its own: the
#: hole this repository nearly shipped was a coverage lane that skipped
#: an actor while the only other lane had been deleted.
DEPENDABOT_GUARD = "github.actor == 'dependabot[bot]'"

#: The actor clause the coverage job excludes, as it appears there.
COVERAGE_ACTOR_CLAUSE = "github.actor != 'dependabot[bot]'"

#: Cache paths holding embedded PostgreSQL binaries. A step restoring one
#: is preparing to start a database, so it belongs to the Dependabot lane
#: and carries that lane's guard. Both are real path entries; the
#: `#`-prefixed lines in the same block scalar are the cache action's
#: comments, not paths.
DATABASE_CACHE_PATHS = ("~/.theseus/postgresql", "~/.cache/pg-embedded/binaries")


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


def test_the_build_job_runs_the_compile_fail_suites_unconditionally() -> None:
    """Compile-fail suites survive the deletion and cannot be skipped.

    Scenario: the compile-fail binaries were excluded from both
    invocations by name, so the build job's own step is the only thing
    that has ever run them. Invariant: it runs both commands as one
    step's entire script, and neither the job nor the step can skip or
    swallow it. A job-level condition skips the gate; a
    `continue-on-error` at either scope runs it and discards the
    verdict, which is the same as not running it.
    """
    job = lanes.build_job()
    assert "if" not in job, (
        "the build job must carry no condition; a skipped job runs no steps "
        "and leaves every step-level assertion here vacuous"
    )
    assert "continue-on-error" not in job, (
        "the build job must not continue on error; a job that swallows its "
        "own failure reports success whatever its steps found"
    )

    compile_fail = [
        step
        for step in lanes.steps_of(job, "build")
        if lanes.script_of(step) == COMPILE_FAIL_SCRIPT
    ]
    assert len(compile_fail) == 1, (
        "expected exactly one build step whose whole script is the two "
        f"compile-fail cargo test commands, found {len(compile_fail)}"
    )
    assert "if" not in compile_fail[0], (
        "the compile-fail step must carry no condition; nothing else in the "
        "repository runs those two suites"
    )
    assert "continue-on-error" not in compile_fail[0], (
        "the compile-fail step must not continue on error; running the suites "
        "and discarding the verdict is the same as not running them"
    )


def test_every_build_step_running_the_suite_is_the_dependabot_lane() -> None:
    """The build job runs the suite for Dependabot and for nobody else.

    Scenario: the coverage job excludes `dependabot[bot]` deliberately,
    to keep dependency automerge off the expensive coverage path, so a
    Dependabot pull request has no backend suite unless this job runs
    one. Invariant: every build step that executes the suite carries the
    Dependabot guard, and the compile-fail step, which runs for
    everyone, is not one of them.

    Both directions matter. An unguarded step here is the duplicate lane
    returning for every contributor; a guard removed from the lane is a
    Dependabot pull request merging with no tests run.
    """
    suite_steps = [
        step
        for step in lanes.build_steps()
        if does.runs_the_suite(step) and lanes.script_of(step) != COMPILE_FAIL_SCRIPT
    ]
    assert suite_steps, (
        "the build job must run the backend suite on the Dependabot lane; "
        "without it a Dependabot pull request runs no tests at all, because "
        "the coverage job excludes that actor"
    )
    unguarded = [
        step.get("name") for step in suite_steps if step.get("if") != DEPENDABOT_GUARD
    ]
    assert unguarded == [], (
        f"these build steps run the suite outside the Dependabot lane: "
        f"{unguarded}; each must carry {DEPENDABOT_GUARD!r} or it is the "
        "duplicate lane returning for every pull request"
    )


def test_the_two_lanes_conditions_are_complementary() -> None:
    """Exactly one lane runs the suite on any pull request.

    Scenario: this is the invariant whose absence let the suite nearly
    disappear for Dependabot. The coverage job excludes an actor; the
    build job's lane includes exactly that actor. Invariant: the two
    clauses are inverses, asserted against each other rather than each
    pinned on its own, so narrowing either without the other fails here.

    A contract that only pinned each condition separately would pass
    with both lanes excluding Dependabot, which is the state this
    repository nearly shipped.
    """
    coverage_condition = lanes.job_named(lanes.ci_workflow(), "coverage").get("if")
    assert COVERAGE_ACTOR_CLAUSE in str(coverage_condition), (
        f"the coverage job's condition {coverage_condition!r} must contain "
        f"{COVERAGE_ACTOR_CLAUSE!r}; the build lane's guard is written as its "
        "inverse and means nothing if the clause it inverts is gone"
    )
    inverse = COVERAGE_ACTOR_CLAUSE.replace("!=", "==")
    assert inverse == DEPENDABOT_GUARD, (
        f"the build lane's guard {DEPENDABOT_GUARD!r} must be the inverse of "
        f"the coverage job's actor clause {COVERAGE_ACTOR_CLAUSE!r}; one actor "
        "excluded from both lanes is an untested pull request"
    )


def test_the_build_jobs_test_tooling_belongs_to_the_dependabot_lane() -> None:
    """Tooling is acquired only where the suite that needs it runs.

    Scenario: the nextest install, the pg_worker install and the
    PostgreSQL warm-up serve the Dependabot lane alone. Invariant: each
    carries that lane's guard. An unguarded one is a normal pull request
    paying for a database it never starts, and is how the deleted lane
    would creep back beside it.
    """
    acquisitions = [
        step for step in lanes.build_steps() if does.acquires_test_tooling(step)
    ]
    assert acquisitions, (
        "the Dependabot lane needs its test runner and database worker; "
        "finding none means the lane cannot run"
    )
    unguarded = [
        step.get("name") for step in acquisitions if step.get("if") != DEPENDABOT_GUARD
    ]
    assert unguarded == [], (
        f"these build steps acquire test tooling outside the Dependabot lane: "
        f"{unguarded}; each must carry {DEPENDABOT_GUARD!r}"
    )


def test_the_build_jobs_database_cache_belongs_to_the_dependabot_lane() -> None:
    """The database archive is restored only where a database is started.

    Scenario: the embedded PostgreSQL cache serves the Dependabot lane
    alone. Invariant: every build cache step listing either archive path
    carries that lane's guard. Appending such a path to a cache with an
    honest purpose is how the restore would return without a step of its
    own to notice.
    """
    restores = [
        step
        for step in lanes.build_steps()
        if set(lanes.cache_paths_of(step)) & set(DATABASE_CACHE_PATHS)
    ]
    unguarded = [
        step.get("name") for step in restores if step.get("if") != DEPENDABOT_GUARD
    ]
    assert unguarded == [], (
        f"these build cache steps carry embedded PostgreSQL binaries outside "
        f"the Dependabot lane: {unguarded}; only a job that starts a database "
        "needs them"
    )
