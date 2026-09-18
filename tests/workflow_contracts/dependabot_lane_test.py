"""Contracts on what the Dependabot test lane is allowed to carry.

Split from `duplicate_test_lane_test.py` when that module crossed the
400-line limit the Python lint gate enforces. The seam is the question:
that module says where the backend suite runs and under what selection,
and this one says that everything serving the Dependabot lane runs only
on that lane, and that the lane has what it needs to run at all.

Both directions are asserted, because each failure is a different fault:

- An unguarded tooling or cache step is the duplicate lane returning for
  every contributor, paying for a database no ordinary pull request
  starts.
- A missing one is a Dependabot pull request downloading every
  PostgreSQL archive again, or failing outright for want of the worker
  binary. The presence assertions exist because the guard assertions on
  their own are satisfied by deleting the steps: an empty list has no
  unguarded member.

That last point is the whole reason the cache case reads the restored
paths rather than intersecting them. Asking whether any step's paths
*meet* the expected set is satisfied when every restore step is gone and
when one of the two paths has been dropped, and either leaves the lane
starting a database it has to download first.
"""

from __future__ import annotations

import ci_lane_reading as lanes
import ci_step_predicates as does
import pytest
from lane_expectations import (
    CACHE_RESTORE_ACTION,
    DATABASE_CACHE_PATHS,
    DEPENDABOT_GUARD,
)


@pytest.fixture(name="document")
def fixture_document() -> dict[str, object]:
    """Return `ci.yml`, parsed once per test.

    The filesystem read lives here, named in every test's signature, so
    no query below is quietly fallible or quietly file-touching.

    Returns
    -------
    dict[str, object]
        The parsed workflow.
    """
    return lanes.load_workflow(lanes.WORKFLOW_PATH)


def test_the_build_jobs_test_tooling_belongs_to_the_dependabot_lane(
    document: dict[str, object],
) -> None:
    """Tooling is acquired only where the suite that needs it runs.

    Scenario: the nextest install, the pg_worker install and the
    PostgreSQL warm-up serve the Dependabot lane alone. Invariant: each
    carries that lane's guard. An unguarded one is a normal pull request
    paying for a database it never starts, and is how the deleted lane
    would creep back beside it.
    """
    acquisitions = [
        step for step in lanes.build_steps(document) if does.acquires_test_tooling(step)
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


def test_the_build_jobs_database_cache_belongs_to_the_dependabot_lane(
    document: dict[str, object],
) -> None:
    """The database archive is restored only where a database is started.

    Scenario: the embedded PostgreSQL cache serves the Dependabot lane
    alone. Invariant: every build cache step listing either archive path
    carries that lane's guard. Appending such a path to a cache with an
    honest purpose is how the restore would return without a step of its
    own to notice.
    """
    restores = [
        step
        for step in lanes.build_steps(document)
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


def test_the_dependabot_lane_restores_every_database_cache_path(
    document: dict[str, object],
) -> None:
    """The lane restores both archive paths, from one guarded restore step.

    Scenario: the embedded PostgreSQL binaries live under two paths, and
    the lane needs both before it starts a cluster. Invariant: exactly
    one guarded `actions/cache/restore` step lists both, and lists them
    together.

    This is the direction the guard contract above cannot see. That one
    asks which steps carry a database path and requires each to be
    guarded, so it passes when every restore step has been deleted and
    when one of the two paths has been dropped: an empty intersection
    has no unguarded member either way. Both of those leave the lane
    downloading the archives it was supposed to have cached.

    The restore action is distinguished from the save action on purpose.
    A lane with only a save step writes a cache it never reads, which
    reports as a cache step in every other contract here and warms
    nothing.
    """
    restores = [
        step
        for step in lanes.build_steps(document)
        if CACHE_RESTORE_ACTION in str(step.get("uses", ""))
        and set(lanes.cache_paths_of(step)) >= set(DATABASE_CACHE_PATHS)
    ]
    assert len(restores) == 1, (
        f"expected exactly one build restore step listing every path in "
        f"{list(DATABASE_CACHE_PATHS)}, found {len(restores)}; without it the "
        "Dependabot lane downloads the embedded PostgreSQL archives on every "
        "run"
    )
    assert restores[0].get("if") == DEPENDABOT_GUARD, (
        f"the database restore step must carry {DEPENDABOT_GUARD!r}; only a "
        "job that starts a database needs the archives"
    )
